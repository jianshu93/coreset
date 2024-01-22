//! Structure and functions to read MNIST fashion database
//! To run the examples change the line :  
//! 
//! const MNIST_FASHION_DIR : &'static str = "/home.1/jpboth/Data/Fashion-MNIST/";
//! 
//! command : mnist_fashion  --algo imp or bmor.
//! 
//! The data can be downloaded in the same format as the FASHION database from:  
//! 
//! <https://github.com/zalandoresearch/fashion-mnist/tree/master/data/fashion>
//! 


use ndarray::s;
use std::fs::OpenOptions;
use std::path::PathBuf;

// to get kmean
use clustering::*;


use std::time::{Duration, SystemTime};
use cpu_time::ProcessTime;

use std::iter::Iterator;
use hnsw_rs::prelude::*;
use coreset::prelude::*;

mod utils;
use utils::{mnistio::*, mnistiter::*};

//============================================================================================

pub struct MnistParams {
    algo : Algo
} // end of MnistParams

impl MnistParams {
    pub fn new(algo : Algo) -> Self {
        MnistParams{algo}
    }
    //
    pub fn get_algo(&self) -> Algo { self.algo}
}

fn marrupaxton<Dist : Distance<f32> + Sync + Send + Clone>(_params :&MnistParams, images : &Vec<Vec<f32>>, labels : &Vec<u8>, distance : Dist) {
    //
    let mpalgo = MettuPlaxton::<f32,Dist>::new(&images, distance);
    let alfa = 0.75;
    let mut facilities = mpalgo.construct_centers(alfa);
    //
    let (entropies, labels_distribution) = facilities.dispatch_labels(&images , labels, None);
    //
    let nb_facility = facilities.len();
    for i in 0..nb_facility {
        let facility = facilities.get_facility(i).unwrap();
        log::info!("\n\n facility : {:?}, entropy : {:.3e}", i, entropies[i]);
        facility.read().log();
        let map = &labels_distribution[i];
        for (key, val) in map.iter() {
            println!("key: {key} val: {val}");
        }
    }
    //
    mpalgo.compute_distances(&mut facilities, &images);
}

//========================================================


fn bmor<Dist : Distance<f32> + Sync + Send + Clone>(_params :&MnistParams, images : &Vec<Vec<f32>>, labels : &Vec<u8>, distance : Dist) {
    //
    // if gamma increases, number of facilities increases.
    // if beta increases , upper bound on cost increases faster so the number of phases decreases
    let beta = 2.;
    let gamma = 2.;
    let mut bmor_algo: Bmor<f32, Dist> = Bmor::new(10, 70000, beta, gamma, distance);
    //
    let ids = (0..images.len()).into_iter().collect::<Vec<usize>>();
    let res = bmor_algo.process_data(images, &ids);
    if res.is_err() {
        std::panic!("bmor failed");
    }
    //
    // do we ask for a supplementary contraction pass
    let contraction = false;
    let mut facilities = bmor_algo.end_data(contraction);
    //
    let (entropies, labels_distribution) = facilities.dispatch_labels(&images , labels, None);
    //
    let nb_facility = facilities.len();
    for i in 0..nb_facility {
        let facility = facilities.get_facility(i).unwrap();
        log::info!("\n\n facility : {:?}, entropy : {:.3e}", i, entropies[i]);
        facility.read().log();
        let map = &labels_distribution[i];
        for (key, val) in map.iter() {
            println!("key: {key} val: {val}");
        }
    }
    //
    facilities.cross_distances();
}

//=====================================================================

use std::cmp::Ordering;

// computes sum of distance to nearest cluster centers
pub fn dispatch_coreset<Dist>(coreset : &CoreSet<f32, Dist>,  c_centers : &Vec<Vec<f32>>, distance : &Dist, images : &Vec<Vec<f32>>) -> f64 
    where Dist : Distance<f32> + Send + Sync + Clone {
    //
    let mut error : f64 = 0.;
    for (id, w_id) in coreset.get_items() {
        if !w_id.is_finite() {
            log::info!("id : {}, w total : {:?}", id, w_id);
            std::panic!();
        }
        // BUG here
        let data = &(images[*id]);
//        assert_eq!(1,0, "data must be data corresponding to id!");
        let (best_c, best_d) : (usize, f32) = (0..c_centers.len()).into_iter()
            .map(|i| (i, distance.eval(data, &c_centers[i])))
            .min_by(| (_,d1), (_,d2)| if d1 < d2 
                    {Ordering::Less} 
                else 
                    {Ordering::Greater })
            .unwrap();
        //
        log::info!(" core id : {} centroid : {}, dist : {:.3e}, weight : {:.3e} ", id, best_c, best_d, w_id);
        if !best_d.is_finite() {
            log::info!("coreset point {:?}, \n cluster center : {:?}", data , c_centers[best_c]);
        }
        assert!(best_d.is_finite());
        // TODO: exponent for dist!!!
        error += (w_id * best_d) as f64;
    }
    //
    error
}

fn coreset1<Dist : Distance<f32> + Sync + Send + Clone>(_params :&MnistParams, images : &Vec<Vec<f32>>, _labels : &Vec<u8>, distance : Dist) {
    // We need to make an iterator producer from data
    let producer = IteratorProducer::new(images);
    // allocate a coreset1 structure
    let beta = 2.;
    let gamma = 2.;
    let k = 10;  // as we have 10 classes, but this gives a lower bound
    let mut core1 = Coreset1::new(k, images.len(), beta, gamma, distance.clone());
    //
    let res = core1.make_coreset(&producer);
    if res.is_err() {
        log::error!("construction of coreset1 failed");
    }
    let coreset = res.unwrap();
    // get some info
    log::info!("coreset1 nb different points : {}", coreset.get_nb_points());
    // TODO: compare errors with kmedoids for L1 and kmeans for L2.
    let dist_name = std::any::type_name::<Dist>();
    log::info!("dist name = {:?}", dist_name);
    match dist_name {
        "hnsw_rs::dist::DistL1" => {
            // going to medoid
            log::info!("doing kmedoid clustering using L1");
            let nb_cluster = 20;
            let mut kmedoids = Kmedoid::new(&coreset, nb_cluster);
            kmedoids.compute_medians();
            let clusters = kmedoids.get_clusters();
            for c in clusters {
                let id = c.get_center_id();
                let label = _labels[id];
                log::info!("cluster center label : {}, cost {:.3e}", label, c.get_cost());
            }
        }

        "hnsw_rs::dist::DistL2" => {
            // going to kmean
            log::info!("doing kmean clustering on whole data .... takes time");
            let nb_iter = 50;
            let nb_cluster = 10;
            let clustering = kmeans(nb_cluster, images, nb_iter);
            // compute error
            let centroids = &clustering.centroids;
            // conver centroids to vectors
            let mut centers = Vec::<Vec<f32>>::with_capacity(nb_cluster);
            for c in centroids {
                let dim = c.dimensions();
                let mut center = Vec::<f32>::with_capacity(dim);
                for i in 0..dim {
                    center.push(c.at(i) as f32);
                }
                centers.push(center);
            }
            let elements = clustering.elements;
            let membership = clustering.membership;
            let mut error = 0.0;
            for i in 0..elements.len() {
                let cluster = membership[i];
                error += distance.eval(&elements[i], &centers[cluster]);
            }
            log::info!("kmean error : {:.3e}", error / images.len() as f32);
            // now we must dispatch our coreset to centers and see what error we have...
            let dispatch_error = dispatch_coreset(&coreset, &centers, &distance, &images);
            log::info!(" coreset dispatching error : {:.3e}", dispatch_error);
        }
        _ => { log::info!("no postprocessing for distance {:?}", dist_name); }
    }
} // end of coreset1


//========================================================

pub fn parse_cmd(matches : &ArgMatches) -> Result<MnistParams, anyhow::Error> {
    log::debug!("in parse_cmd");
    if matches.contains_id("algo") {
        println!("decoding argument algo");
        let algoname = matches.get_one::<String>("algo").expect("");
        log::debug!(" got algo : {:?}", algoname);
        match algoname.as_str() {
            "imp" => {
                let params = MnistParams::new(Algo::IMP);
                return Ok(params);
            },
            "bmor" => {
                let params = MnistParams::new(Algo::BMOR);
                return Ok(params);
            }
            "coreset1" => {
                let params = MnistParams::new(Algo::CORESET1);
                return Ok(params);
            }  
            //
            _           => {
                log::error!(" algo must be imp or bmor or coreset1 ");
                std::process::exit(1);
            }
        }
    }
    //
    return Err(anyhow::anyhow!("bad command"));
} // end of parse_cmd



//========================================================

use clap::{Arg, ArgMatches, ArgAction, Command};



const MNIST_FASHION_DIR : &'static str = "/home/jpboth/Data/ANN/Fashion-MNIST/";

pub fn main() {
    //
    let _ = env_logger::builder().is_test(true).try_init();
    //
    log::info!("running mnist_fashion");
    //
    let matches = Command::new("mnist_fashion")
    //        .subcommand_required(true)
            .arg_required_else_help(true)
            .arg(Arg::new("algo")
                .required(true)
                .long("algo")    
                .action(ArgAction::Set)
                .value_parser(clap::value_parser!(String))
                .required(true)
                .help("expecting a algo option imp, bmor "))
        .get_matches();
    //
    let mnist_params = parse_cmd(&matches).unwrap();
    //
    let mut image_fname = String::from(MNIST_FASHION_DIR);
    image_fname.push_str("train-images-idx3-ubyte");
    let image_path = PathBuf::from(image_fname.clone());
    let image_file_res = OpenOptions::new().read(true).open(&image_path);
    if image_file_res.is_err() {
        println!("could not open image file : {:?}", image_fname);
        return;
    }
    let mut label_fname = String::from(MNIST_FASHION_DIR);
    label_fname.push_str("train-labels-idx1-ubyte");
    let label_path = PathBuf::from(label_fname.clone());
    let label_file_res = OpenOptions::new().read(true).open(&label_path);
    if label_file_res.is_err() {
        println!("could not open label file : {:?}", label_fname);
        return;
    }
    let mut images_as_v:  Vec::<Vec<f32>>;
    let mut labels :  Vec<u8>;
    {
        let mnist_train_data  = MnistData::new(image_fname, label_fname).unwrap();
        let images = mnist_train_data.get_images();
        labels = mnist_train_data.get_labels().to_vec();
        let( _, _, nbimages) = images.dim();
        //
        images_as_v = Vec::<Vec<f32>>::with_capacity(nbimages);
        for k in 0..nbimages {
            // we convert to float normalized 
            let v : Vec<f32> = images.slice(s![.., .., k]).iter().map(|v| *v as f32 / (28. * 28.)).collect();
            images_as_v.push(v);
        }
    } // drop mnist_train_data
    // now read test data
    let mut image_fname = String::from(MNIST_FASHION_DIR);
    image_fname.push_str("t10k-images-idx3-ubyte");
    let image_path = PathBuf::from(image_fname.clone());
    let image_file_res = OpenOptions::new().read(true).open(&image_path);
    if image_file_res.is_err() {
        println!("could not open image file : {:?}", image_fname);
        return;
    }
    let mut label_fname = String::from(MNIST_FASHION_DIR);
    label_fname.push_str("t10k-labels-idx1-ubyte");
    let label_file_res = OpenOptions::new().read(true).open(&label_path);
    if label_file_res.is_err() {
        println!("could not open label file : {:?}", label_fname);
        return;
    }
    {
        let mnist_test_data  = MnistData::new(image_fname, label_fname).unwrap();
        let test_images = mnist_test_data.get_images();
        let mut test_labels = mnist_test_data.get_labels().to_vec();
        let( _, _, nbimages) = test_images.dim();
        let mut test_images_as_v = Vec::<Vec<f32>>::with_capacity(nbimages);
        //
        for k in 0..nbimages {
            let v : Vec<f32> = test_images.slice(s![.., .., k]).iter().map(|v| *v as f32 / (28.*28.)).collect();
            test_images_as_v.push(v);
        }
        labels.append(&mut test_labels);
        images_as_v.append(&mut test_images_as_v);
    } // drop mnist_test_data

    //
    // test mettu-plaxton or bmor algo
    //
    let cpu_start = ProcessTime::now();
    let sys_now = SystemTime::now();
    //
    let distance = DistL1::default();
    match mnist_params.get_algo() {
        Algo::IMP   => {
            marrupaxton(&mnist_params, &images_as_v, &labels, distance)
        }
        Algo::BMOR   => {
            bmor(&mnist_params, &images_as_v, &labels, distance);
        } 
        Algo::CORESET1 => {
            coreset1(&mnist_params, &images_as_v, &labels, distance);
        }   
    }
    //
    let cpu_time: Duration = cpu_start.elapsed();
    println!("  sys time(ms) {:?} cpu time(ms) {:?}", sys_now.elapsed().unwrap().as_millis(), cpu_time.as_millis());
} // end of main


//============================================================================================



#[cfg(test)]

mod tests {


use super::*;

// test and compare some values obtained with Julia loading

#[test]
fn test_load_mnist_fashion() {
    let mut image_fname = String::from(MNIST_FASHION_DIR);
    image_fname.push_str("train-images-idx3-ubyte");
    let image_path = PathBuf::from(image_fname.clone());
    let image_file_res = OpenOptions::new().read(true).open(&image_path);
    if image_file_res.is_err() {
        println!("could not open image file : {:?}", image_fname);
        return;
    }

    let mut label_fname = String::from(MNIST_FASHION_DIR);
    label_fname.push_str("train-labels-idx1-ubyte");
    let label_path = PathBuf::from(label_fname.clone());
    let label_file_res = OpenOptions::new().read(true).open(&label_path);
    if label_file_res.is_err() {
        println!("could not open label file : {:?}", label_fname);
        return;
    }

    let _mnist_data  = MnistData::new(image_fname, label_fname).unwrap();
    // check some value of the tenth images

} // end test_load


}  // end module tests