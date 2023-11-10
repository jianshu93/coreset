//! implement facility management.  
//! Each facility maintain weights of data itmes dispatched to it and cost contribution
//! 

use anyhow::*;

use serde::{Serialize, Deserialize, de::DeserializeOwned};


use ndarray::Array2;
use quantiles::ckms::CKMS;     // we could use also greenwald_khanna

use rayon::prelude::*;

use parking_lot::RwLock;
use std::sync::Arc;

use std::collections::HashMap;

use hnsw_rs::dist::*;

/// A facility is a center (or point in data) that correspond to a k medoid point.  
/// The struture stores the data point which serve as a center, the sum of points weight 
/// attached to this point and the cost (distance between data points and center multiplied by point's weight)
#[derive(Clone, Serialize, Deserialize)]
pub struct Facility<T: Send+Sync+Clone> {
    // rank in data
    d_rank : usize,
    // facility location
    center : Vec<T>,
    // weight (how many points it represents)
    weight : f64,
    //
    cost : f64,
    //
} // end of Facility



impl<T: Send+Sync+Clone> Facility<T> {

    /// creates a facility, around a point characteristics,
    /// TODO: set its  rank as an option
    /// As the point could be different from data as in kmean we do not set weight.
    /// So an explicit insertion with method insert must be done when creation facility is 
    /// mean to also insert
    pub fn new(d_rank : usize, center : &[T]) -> Self {
        Facility{d_rank,center : center.to_vec(), weight : 0. , cost : 0.}
    }

    pub fn get_position(&self) -> &Vec<T> {
        return &self.center;
    }

    /// get data rank this facility is centered on
    pub fn get_dataid(&self) -> usize {
        self.d_rank
    }

    /// return sum of points' weight dipatched to this center
    pub fn get_weight(&self) -> f64 {
        self.weight
    }

    #[cfg_attr(doc, katexit::katexit)]
    /// return cost carried by this facility $f$ i.e :  
    ///    $ cost(f) = \sum_{p \in f} w(p) * dist(p,f) $
    pub fn get_cost(&self) -> f64 {
        self.cost
    }

    // This function increments weight and cost related to a facility
    pub(crate) fn insert(&mut self, weight : f64, dist : f32) {
        self.weight += weight;
        self.cost += dist as f64 * weight;
    }

    pub fn log(&self) {
        log::info!("facility , d_rank : {:?}  weight : {:.4e},  cost : {:.3e}  cost/weight : {:.3e}", self.d_rank, self.weight, self.cost, self.cost/self.weight);
    }
} // end of block Facility


//===================================================================================


/// Describes the list of facility (or centers created). 
///  
/// As we want parallel access, running concurrently on all data we need Arc RwLock stuff
#[derive(Clone)]
pub struct Facilities<T : Send+Sync+Clone, Dist : Distance<T> > {
    centers : Vec<Arc<RwLock<Facility<T>>>>,
    //
    distance : Dist,
}

impl <T:Send+Sync+Clone, Dist : Distance<T> + Send + Sync > Facilities<T, Dist> {

    /// to be allocated , size should be log(nb_data)
    pub fn new(size : usize, distance : Dist) -> Self {
        let centers = Vec::<Arc<RwLock<Facility<T>>>>::with_capacity(size);
        Facilities{centers, distance}
    }

    /// return number of facility
    pub fn len(&self) -> usize {
        return self.centers.len();
    }

    /// total weight already inserted
    pub fn get_weight(&self) -> f64 {
        return self.centers.iter().map(|f| f.read().get_weight()).sum();
    }


    // useful in algorithm bmor when we need to reinitialize
    pub(crate) fn clear(&mut self) {
        self.centers.clear();
    }


    // access to internal representation
    pub(crate) fn get_vec(&self) -> &Vec<Arc<RwLock<Facility<T>>>> {
        &self.centers
    }


    /// return true if there is a facility around point at distance less than dmax
    pub fn match_point(&self, point : &Vec<T>, dmax : f32, distance : &Dist) -> bool {
        //
        for f in &self.centers {
            if distance.eval(f.read().get_position(), point) <= dmax {
                return true;
            }
        }
        return false;
    } // end of match_facility


    ///
    pub(crate) fn insert(&mut self, facility : Facility<T>) {
        self.centers.push(Arc::new(RwLock::new(facility)));
        //
        log::trace!("Facilities: facility insertion nb facilities : {}", self.centers.len());
    }


    /// retrieve facility by rank if rank is Ok
    pub fn get_facility(&self, rank : usize) -> Option<&Arc<RwLock<Facility<T>>>> {
        if rank >= self.centers.len() {
            return None;
        }
        else {
            return Some(&self.centers[rank]);
        }
    }

    /// retrieve facility of given rank , and returns a clone (to avoid synchro stuff)
    /// useful for easy final analysis
    pub fn get_cloned_facility(&self, rank : usize) -> Option<Facility<T>> {
        if rank >= self.centers.len() {
            return None;
        }
        else {
            return Some(self.centers[rank].read().clone());
        }        
    } // end of get_cloned_facility


    /// return rank of nearest facility, returns rank of facility and distance to it
    pub fn get_nearest_facility(&self, data : &[T]) -> anyhow::Result<(usize, f32)> {
        let mut dist = f32::INFINITY;
        let mut rank_f : i32 = -1;
        if self.centers.len() == 0 {
            return Err(anyhow!("Empty facility"));
        }
        // TODO: can be // if many centers
        for i in 0..self.centers.len() {
            let f_i = self.get_facility(i).unwrap().read();
            let center_i = f_i.get_position(); 
            let d_i = self.distance.eval(center_i, data);
            if d_i <= dist {
                dist = d_i;
                rank_f = i as i32;
            }
        }
        //
        return Ok((rank_f as usize, dist));
    } // end of get_nearest_facility

    
    /// a function to log info on dist and cost inside facilities
    /// - level = 0, will log total weight and total cost summed over facilities
    /// - level = 1 it will log weight and cost of each facility.
    pub fn log(&self, level : usize) {
        let mut total_weight = 0.;
        let mut total_cost = 0.;
        for f in &self.centers {
            let f_access = f.read();
        if level == 1 {
                f_access.log();
            }
            total_cost += f_access.get_cost();
            total_weight += f_access.get_weight();
        }
        log::info!("\n\n sum of facilities weight : {:.3e}, total cost : {:.3e}", total_weight, total_cost);
        log::info!("\n *************************************************");
    } // end of log 



    /// affect each point to its facility and compute cost of each facility.  
    /// Not all algos maintain weight and cost consistently all the way, sometimes facilities are created without
    /// searching all points in it. So we need to be able do the dispatch afterwards.  
    /// **Bmor algorithm dispatch points on the fly so it computes an upper bound of the cost**  
    /// **The function can nevertheless be called a posteriori to get a tighter bound on cost**  
    /// This function returns global cost and vector of weight by facility
    #[allow(unused)]
    pub fn dispatch_data(&mut self, data : &Vec<&Vec<T>>, weights : Option<&Vec<f32>>) -> f64 {
        //
        log::info!("in facilities::dispatch_data");
        //
        if weights.is_some() {
            assert_eq!(data.len(), weights.unwrap().len());
        }
        //
        let nb_facility = self.centers.len();
        for i in 0..nb_facility {
            self.centers[i].write().cost = 0.;
            self.centers[i].write().weight = 0.;
        }
        //
        let dispatch_i = | item : usize | {
            // get facility rank and weight
            let (facility, dist) = self.get_nearest_facility(&data[item]).unwrap();
            let weight = if weights.is_none() { 1. } else { weights.unwrap()[item] as f64};
            let cost_incr = dist as f64 * weight;
            let mut facility = self.centers[facility].write();
            facility.weight += weight;
            facility.cost += cost_incr;
        };
        //
        (0..data.len()).into_par_iter().for_each( |item| dispatch_i(item));
        //
        let mut global_cost = 0_f64;
        let mut total_weight = 0.;
        for i in 0..nb_facility {
            global_cost += self.centers[i].read().cost;
            total_weight += self.centers[i].read().weight;
        }
        //
        println!("\n\n total weight collected in facilities : {:.3e}, total cost : {:.3e}", total_weight, global_cost);
        println!("\n **************************************************************************");
        //
        global_cost
    } // end of dispatch_data




    /// If we have labelled data we can store labels counts affected to each facility.  
    /// This function dispatch data and lables into facilities and returns total cost and a vector of counts for each label occuring in a Facility.  
    /// It computes for each facililty label distribution, entropy of distribution and can be used to check clustering. 
    /// **This methods can be called after processing all the data**.     
    /// Returns Vector of label distribution entropy by facility and distribution as a HashMap
    pub fn dispatch_labels<L : PartialEq + Eq + Copy + std::hash::Hash + Sync + Send>(& mut self, data : &Vec<Vec<T>>, labels : &Vec<L>, weights : Option<&Vec<f32>>) -> (Vec<f64>, Vec<HashMap<L, u32>>) {
        //
        log::info!("dispatch_labels");
        //
        type SafeHashMap<L> = Arc<RwLock<HashMap<L, u32>>>;
        assert_eq!(data.len(), labels.len());
        //
        let nb_facility = self.centers.len();
        let mut label_distribution = Vec::<SafeHashMap<L>>::with_capacity(nb_facility);

        for i in 0..nb_facility {
            // reinitialize weights and cost of facilities
            self.centers[i].write().cost = 0.;
            self.centers[i].write().weight = 0.;
            // allocate hashmaps
            let newmap = HashMap::<L, u32>::with_capacity(data.len() / (2* nb_facility));
            label_distribution.push(Arc::new(RwLock::new(newmap)));
        }
        //
        let dispatch_i = | i : usize | {
            // find facility
            let (itemf, dist) = self.get_nearest_facility(&data[i]).unwrap();
            // dispatch data
            let weight = if weights.is_none() { 1. } else { weights.unwrap()[itemf] as f64};
            let cost_incr = dist as f64 * weight;
            {
                let mut facility = self.centers[itemf].write();
                facility.weight += weight;
                facility.cost += cost_incr;
            }
            // dispatch label
            {   // write lock
                let mut distribution = label_distribution[itemf].write();
                if let Some(count) = distribution.get_mut(&labels[i]) {
                    *count += 1;
                }
                else {
                    distribution.insert(labels[i], 1);
                }
            }
        };
        //
        log::info!("computing global cost and weights");
        (0..data.len()).into_par_iter().for_each( |item| dispatch_i(item));
        // recompute globlas cost and weight
        let mut global_cost = 0_f64;
        let mut total_weight = 0.;
        for i in 0..nb_facility {
            global_cost += self.centers[i].read().cost;
            total_weight += self.centers[i].read().weight;
        }
        //
        println!("\n\n total weight collected in facilities : {:.3e}, total cost : {:.3e}", total_weight, global_cost);
        println!("\n **************************************************************************");
        //
        // We can compute entropy distribution
        //
        log::info!("computing label distribution entropy");
        let mut entropies = Vec::<f64>::with_capacity(nb_facility);
        for i in 0..nb_facility {
            let distribution = label_distribution[i].read();
            let mut mass = 0.0f64;
            let nb_label = distribution.len();
            let mut weights = Vec::<f64>::with_capacity(nb_label);
            let mut entropy = 0.;
            for item in distribution.iter() {
                assert!(*item.1 > 0);
                weights.push(*item.1 as f64);
                mass += *item.1 as f64;
                entropy -= (*item.1 as f64) * (*item.1 as f64).ln();
            }
            entropy = entropy / mass + mass.ln();
            entropies.push(entropy);
        }
        // Construct global weighted entropy measure 
        let mut global_entropy = 0.;
        let mut total_weight = 0.;
        for i in 0..nb_facility {
            let facility = self.centers[i].read();
            let weight = facility.get_weight();
            total_weight += weight;
            global_entropy +=  weight * entropies[i];
        }
        global_entropy /= total_weight;
        println!("\n\n mean of entropies : {:.3e}, total weight : {:.3e}", global_entropy, total_weight);
        println!("\n **************************************************************************");
        //
        let mut simple_label_distribution = Vec::<HashMap<L,u32>>::with_capacity(nb_facility);
        for i in 0..nb_facility {
            simple_label_distribution.push(label_distribution[i].read().clone());
        }
        //
        return (entropies, simple_label_distribution);
    } // end of dispatch_labels


    /// extract facility centers and associated weight for possible other clustering step
    pub fn into_weighted_data(&self) -> Vec<(f64, Vec<T>)> {
        log::info!("facility::into_weighted_data");
        //
        let nb_facility = self.len();
        let mut weighted_data = Vec::<(f64, Vec<T>)>::with_capacity(nb_facility);
        for i in 0..nb_facility {
            let facility = self.get_facility(i).unwrap().read();
            let weight = facility.get_weight();
            let pos = facility.get_position();
            weighted_data.push((weight, pos.clone()));
        }
        weighted_data
    } // end of into_weighted_data


    /// returns weights as a Vec\<f32\> and data. Same as [into_weighted_data](Self::into_weighted_data()) but in another format
    pub fn get_weights_and_data(&self) -> (Vec<f32>, Vec<Vec<T>>) {
        let nb_facility = self.len();
        let mut data = Vec::<Vec<T>>::with_capacity(nb_facility);
        let mut weights = Vec::<f32>::with_capacity(nb_facility);
        for i in 0..nb_facility {
            let facility = self.get_facility(i).unwrap().read();
            weights.push(facility.get_weight() as f32);
            let pos = facility.get_position();
            data.push(pos.clone());
        }
        (weights, data)       
    }


        // TODO: useful?
    /// computes distances between facility
    pub fn cross_distances(&self) {
        let nb_facility = self.centers.len();
        let mut distances = Array2::<f32>::zeros((nb_facility , nb_facility));
        let mut q_dist = CKMS::<f32>::new(0.01);

        if nb_facility <= 1 {
            log::error!("facility::cross_distances, only one facility");
            return;
        }
        for i in 0..nb_facility {
            let f_i = self.get_facility(i).unwrap().read();
            let center_i = f_i.get_position();
            for j in 0..nb_facility {
                if i!=j {
                    let f_j = self.get_facility(j).unwrap().read();
                    distances[[i,j]] = self.distance.eval(center_i, f_j.get_position());
                    q_dist.insert(distances[[i,j]]);
                }
            }
        }
        //
        println!("\n inter facility distances quantiles : ");
        println!("\n distance quantiles at  0.01 : {:.2e}, 0.05 :  {:.2e},   0.1 : {:.2e} , 0.5 : {:.2e}, 0.75 :  {:.2e} ", 
        q_dist.query(0.01).unwrap().1, q_dist.query(0.05).unwrap().1, q_dist.query(0.1).unwrap().1, q_dist.query(0.5).unwrap().1,  q_dist.query(0.75).unwrap().1);
        //
        log::debug!("\n cross distances : {:.3e}", distances);
    } // end of cross_distances

} // end of impl block Facilities
