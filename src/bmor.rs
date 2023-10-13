//! adaptation of Streaming k-means on well clustered data
//! Braverman Meyerson Ostrovski Roytman ACM-SIAM 2011
//! 
//! 
//! 
//! 

use std::marker::PhantomData;

use parking_lot::RwLock;
use std::sync::Arc;

use rand_xoshiro::Xoshiro256PlusPlus;
use rand_xoshiro::rand_core::SeedableRng;

use rand::distributions::{Distribution,Uniform};

use hnsw_rs::dist::*;

use crate::facility::*;



#[derive(Clone)]
pub struct BmorState<T:Send+Sync+Clone, Dist : Distance<T> > {
    // (1+logn)k
    oneplogn : usize,
    // nb iterations (phases)
    phase : usize,
    //
    li : f64,
    // at each phase we have an upper bound for cost.
    phase_cost_upper : f64,
    // upper bound on number of facilities
    facility_bound : usize, 
    // current centers, associated to rank in stream (or in data) and weight (nb points in facility)
    centers : Facilities<T, Dist>,
    // sum of absolute value (some algos have <0 weights) of inserted weight
    absolute_weight : f64,
    // total cost
    total_cost : f64,
    //
    nb_inserted : usize,
    //
    rng : Xoshiro256PlusPlus,
    //
    unif : Uniform::<f64>,
} // end of 


impl<T:Send+Sync+Clone, Dist : Distance<T> + Clone + Sync> BmorState<T, Dist> {

    pub(crate) fn new(k : usize, nbdata : usize, phase : usize, alloc_size : usize, upper_cost : f64, facility_bound : usize, distance : Dist) -> Self {
        let centers = Facilities::<T, Dist>::new(alloc_size, distance);
        let unif = Uniform::<f64>::new(0., 1.);
        let rng = Xoshiro256PlusPlus::seed_from_u64(1454691);
        let oneplogn = (1 + nbdata.ilog2()) as usize * k;
        let li = 1.0f64;
        BmorState{oneplogn, phase, li, phase_cost_upper : upper_cost, facility_bound, centers, absolute_weight : 0., total_cost : 0., nb_inserted : 0, rng, unif}
    }


    pub fn get_facilities(&self) -> &Facilities<T, Dist> {
        return &self.centers
    }

    pub fn get_mut_facilities(&mut self) -> &mut Facilities<T, Dist> {
        return &mut self.centers
    }
    // get current phase num of processing
    pub(crate) fn get_phase(&self) -> usize {
        self.phase
    }

    pub(crate) fn get_li(&self) -> f64 {
        self.li
    }


    pub(crate) fn get_nb_inserted(&self) -> usize {
        self.nb_inserted
    }

    pub(crate) fn get_unif_sample(&mut self) -> f64 {
        self.unif.sample(&mut self.rng)
    }

    pub(crate) fn get_phase_cost_bound(&self) -> f64 {
        self.phase_cost_upper
    }

    /// get sum of absolute value of weights inserted
    pub(crate) fn get_weight(&self) -> f64 {
        self.absolute_weight
    }

    /// get sum of absolute value of weights inserted
    pub(crate) fn get_cost(&self) -> f64 {
        self.total_cost
    }    

    /// get nearest center/facility of a point
    pub fn get_nearest_center(&self, point : &[T]) -> Option<(&Arc<RwLock<Facility<T>>>, f32) >
        where T : Send + Sync,  Dist : Sync {
        //
        let nb_facility = self.centers.len();
        //
        if nb_facility == 0 {
            return None;
        }
        // get nearest facilty
        let (rank,dist) = self.centers.get_nearest_facility(point).unwrap();
        //
        return Some( (self.centers.get_facility(rank).unwrap(), dist));
    } // end of get_nearest_center


    /// insert into an already existing facility
    /// return true if all is OK, false if costs or number of facilities got too large
    fn update(&mut self, rank_id : usize, point : &[T], weight : f64) -> bool {
        //
        log::debug!("in BmorState::update rank_id: {:?}", rank_id);
        //
        let dist_to_nearest : f32;
        let nearest_facility : Arc<RwLock<Facility<T>>>;
        {
            let nearest_facility_res = self.get_nearest_center(point);
            if nearest_facility_res.is_none() {
                log::error!("internal error, update did not find nearest facility");
                std::process::exit(1);
            }
            let nearest_center =  nearest_facility_res.unwrap();
            dist_to_nearest = nearest_center.1;
            nearest_facility = nearest_center.0.clone();
        }
        // take into account f factor
        if self.get_unif_sample() < (weight * dist_to_nearest as f64 * self.oneplogn as f64 / self.li) {
            // we create a new facility. No cost increment
            let mut new_f = Facility::<T>::new(rank_id, point);
            new_f.insert(weight as f64,dist_to_nearest);
            self.centers.insert(new_f);
            log::debug!("in BmorState::update  creating new facility around {}, nb_facilities : {}", rank_id, self.centers.len());
        }
        else {
            log::debug!("in BmorState::update rank_id: {:?}, inserting in old facility dist : {:.3e}", rank_id, dist_to_nearest);
            nearest_facility.write().insert(weight, dist_to_nearest);
            self.total_cost += weight.abs() as f64 * dist_to_nearest as f64;
        }
        // we increments weight monitoring and number of insertions
        self.absolute_weight += weight.abs() as f64;
        self.nb_inserted += 1;
        // check if we are above constraints
        if self.total_cost > self.phase_cost_upper || self.centers.len() > self.facility_bound {
            log::info!("constraint violation");
            self.log();
            return false
        }
        else {
            return true;
        }
    } // end of update


    // reinitialization. (upper cost rescaling)
    pub(crate) fn reinit(&mut self, li : f64, phase_cost_upper : f64) {
        self.phase += 1;
        self.phase_cost_upper = phase_cost_upper as f64;
        self.li = li;
        self.centers.clear();
        self.absolute_weight = 0.;
        self.total_cost = 0.;
    }

    pub(crate) fn log(&self) {
        log::info!("\n\n BmorState::log_state");
        log::info!("\n nb facilities : {:?}", self.centers.len());
        log::info!("\n weight : {:.3e}   cost {:.3e}", self.get_weight(), self.get_cost());
        log::info!("\n nb facility max : {:?}, upper cost bound : {:.3e}", self.facility_bound, self.get_phase_cost_bound());
        log::info!("\n nb total insertion : {:?}  nb_phases: {:?}", self.get_nb_inserted(), self.phase + 1);
    }
} // end of impl block BmorState





pub struct Bmor<T, Dist> {
    // base number of centers expected
    k : usize,
    //
    nbdata_expected : usize,
    // cost multiplicative factor for upper bound of accepted cost at each phase.
    beta : f64,
    //  slackness parameters for cost and number of centers accepted
    gamma : f64,
    //
    distance : Dist,
    //
    _t : PhantomData<T>,
}  // end of struct Bmor



impl <T : Send + Sync + Clone, Dist> Bmor<T, Dist> 
    where  Dist : Distance<T> + Clone + Sync {

    /// - k: number of centers
    /// - nbdata : nb data expected,
    /// - gamma 
    pub fn new(k: usize, nbdata : usize, beta : f64, gamma : f64, distance :  Dist) -> Self {
        // TODO: to be adapted?
        Bmor{k, nbdata_expected : nbdata, beta, gamma, distance, _t : PhantomData::<T> }
    }


    pub fn process_block(&self, data : &Vec<Vec<T>>) -> BmorState<T, Dist> {
        //
        let nb_centers_bound = (self.gamma * (1. + self.nbdata_expected.ilog2() as f64) * self.k as f64).trunc() as usize; 
        let upper_cost = self.gamma;
        let mut state = BmorState::<T, Dist>::new(self.k, self.nbdata_expected, 0, nb_centers_bound as usize, 
                    upper_cost as f64, nb_centers_bound, self.distance.clone());
        //
        let weighted_data: Vec<(f64, &Vec<T>, usize)> = (0..data.len()).into_iter().map( |i| (1.,&data[i],i)).collect();
        self.process_weighted_block(&mut state, &weighted_data);
        state.log();
        state.get_facilities().log();
        //
        return state;
    } // end of process_block



    // This method can do block processing as dispatched by 
    // recurring processing
    pub fn process_weighted_block(&self, state : &mut BmorState<T, Dist>, data : &Vec<(f64,&Vec<T>, usize)>) {
        //
        log::debug!("entering process_weighted_block, phase : {:?}, nb data : {}", state.get_phase(), data.len());
        //
        for d in data {
            // TODO: now we use rank as rank_id (sufficicent for ordered ids)
            log::debug!("treating rank_id : {:?}, weight : {:.4e}", d.2, d.0);
            let add_res = self.add_data(state, d.2, &d.1, d.0);
            if !add_res {
                // allocate new state
                log::debug!("recycling facilities, incrementing upper bound for cost, nb_facilities : {:?}", state.get_facilities().len());
                let old_state = state.clone();
                // recycle facilitites in process adding them
                let weighted_data : Vec<(f64,Vec<T>, usize)>;
                weighted_data = state.centers.get_vec().iter().map(|f| (f.read().get_weight(), f.read().get_position().clone(), f.read().get_dataid())).collect();
                assert!(weighted_data.len() > 0);
                let weighted_ref_data : Vec<(f64,&Vec<T>, usize)> = weighted_data.iter().map(|wd| (wd.0, &wd.1, wd.2)  ).collect();
                assert!(weighted_ref_data.len() > 0);
                state.reinit(self.beta * old_state.get_li() as f64, self.beta * old_state.get_phase_cost_bound());
                self.process_weighted_block(state, &weighted_ref_data);
            }
        }
    } // end of process_weighted_block


    // This function return true except if we got beyond bound for cost or number of facilities
    // The data added can be a facility extracted during a preceding phase
    pub(crate) fn add_data(&self, state : &mut BmorState<T, Dist>, rank_id : usize, data : &Vec<T>, weight : f64) -> bool {
        let facilities = state.get_mut_facilities();
        // get nearest facility or open facility
        if facilities.len() <= 0 {
            log::debug!("Bmor::add_data creating facility rank_id : {} with weight : {:.3e}", rank_id, weight);
            let mut new_f = Facility::<T>::new(rank_id, data);
            new_f.insert(weight, 0.);
            facilities.insert(new_f);
            // we update global state here in facility creation case
            state.nb_inserted += 1;
            state.absolute_weight += weight as f64;
            return true;
        }
        // we already have a facility we update state
        let status = state.update(rank_id, data, weight);
        //
        return status;
    }


} // end of impl block Bmor
