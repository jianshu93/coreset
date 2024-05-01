//! This binary is dedicated to coreset computations on data stored in Hnsw created by crate [hnsw_rs](https://crates.io/crates/hnsw_rs)
//!
//! command is :hnscore  --dir (-d) dirname  --fname (-f) hnswname  --typename (-t) typename [--beta b] [--gamma g]
//!
//! - dirname : directory where hnsw files reside
//! - hnswname : name used for naming the 2 hnsw related files: name.hnsw.data and name.hnsw.graph
//! - typename : can be u16, u32, u64, f32, f64, i16, i32, i64
//!
//! The coreset command takes as arguments:
//! - beta:
//! - gamma:
//!
//! Note: It is easy to add any adhoc type T  by adding a line in [get_datamap()].  
//! The only constraints on T comes from hnsw and is T: 'static + Clone + Sized + Send + Sync + std::fmt::Debug

#![allow(unused)]

use std::fs::OpenOptions;
use std::path::PathBuf;

use cpu_time::ProcessTime;
use std::time::{Duration, SystemTime};

use coreset::prelude::*;
use std::iter::Iterator;

use clap::{Arg, ArgAction, ArgMatches, Command};
use std::default::Default;

use fromhnsw::getdatamap::get_typed_datamap;
use hnsw_rs::datamap::*;

//========================================
// Parameters

#[derive(Debug, Clone)]
struct HcoreParams {
    path: HnswParams,
    corearg: CoresetParams,
}
#[derive(Debug, Clone)]
struct HnswParams {
    dir: String,
    hname: String,
    typename: String,
}

impl HnswParams {
    pub fn new(hdir: &String, hname: &String, typename: &String) -> Self {
        HnswParams {
            dir: hdir.clone(),
            hname: hname.clone(),
            typename: typename.clone(),
        }
    }
}

//
/// Coreset parameters
#[derive(Copy, Clone, Debug)]
struct CoresetParams {
    beta: f32,
    gamma: f32,
}

impl CoresetParams {
    fn new(beta: f32, gamma: f32) -> CoresetParams {
        CoresetParams { beta, gamma }
    }
    //
    fn get_beta(&self) -> f32 {
        self.beta
    }

    //
    fn get_gamma(&self) -> f32 {
        self.gamma
    }
}

impl Default for CoresetParams {
    fn default() -> Self {
        CoresetParams {
            beta: 2.,
            gamma: 2.,
        }
    }
}

#[derive(Clone, Debug)]
struct HnswCore {
    // paths
    hparams: HnswParams,
    // algo parameters
    coreparams: CoresetParams,
}

//===========================================================

fn parse_coreset_cmd(matches: &ArgMatches) -> Result<CoresetParams, anyhow::Error> {
    log::debug!("in  parse_coreset_cmd");
    let mut params = CoresetParams::default();
    params.beta = *matches.get_one::<f32>("beta").unwrap();
    params.gamma = *matches.get_one::<f32>("gamma").unwrap();
    //
    log::info!("got CoresetParams : {:?}", params);
    //
    return Ok(params);
}

//============================================================================================

/// This function dispatch its call to get_typed_datamap::\<T\> according to type T
/// The cuurent function dispatch to u16, u32, u64, i32, i64, f32 and f64 according to typename.
/// For another type, the functio is easily modifiable.  
/// The only constraints on T comes from hnsw and is T: 'static + Clone + Sized + Send + Sync + std::fmt::Debug
pub fn get_datamap(directory: String, basename: String, typename: &str) -> anyhow::Result<DataMap> {
    //
    let _datamap = match &typename {
        &"u16" => get_typed_datamap::<u16>(directory, basename),
        &"u32" => get_typed_datamap::<u32>(directory, basename),
        &"u64" => get_typed_datamap::<u64>(directory, basename),
        &"f32" => get_typed_datamap::<f32>(directory, basename),
        &"f64" => get_typed_datamap::<f64>(directory, basename),
        &"i32" => get_typed_datamap::<i32>(directory, basename),
        &"i64" => get_typed_datamap::<i64>(directory, basename),
        _ => {
            log::error!(
                "get_datamap : unimplemented type, type received : {}",
                typename
            );
            std::panic!("get_datamap : unimplemented type");
        }
    };
    std::panic!("not yet");
}

//===========================================================

fn main() {
    //
    let _ = env_logger::builder().is_test(true).try_init();
    //
    log::info!("running hnswcore");
    //
    let hparams: HnswParams;
    let core_params: CoresetParams;

    //
    let params: CoresetParams;
    let coresetcmd = Command::new("coreset")
        .arg(
            Arg::new("beta")
                .required(false)
                .short('b')
                .long("beta")
                .default_value("2.0")
                .action(ArgAction::Set)
                .value_parser(clap::value_parser!(f32))
                .help("beta"),
        )
        .arg(
            Arg::new("gamma")
                .required(false)
                .short('g')
                .long("gamma")
                .default_value("2.0")
                .action(ArgAction::Set)
                .value_parser(clap::value_parser!(f32))
                .help("gamma"),
        );
    //
    // global command
    // =============
    //
    let matches = Command::new("hnswcore")
        .arg_required_else_help(true)
        .arg(
            Arg::new("dir")
                .long("dir")
                .short('d')
                .action(ArgAction::Set)
                .value_parser(clap::value_parser!(String))
                .required(true)
                .help("expecting a directory name"),
        )
        .arg(
            Arg::new("fname")
                .long("fname")
                .short('f')
                .action(ArgAction::Set)
                .value_parser(clap::value_parser!(String))
                .required(true)
                .help("expecting a file  basename"),
        )
        .arg(
            Arg::new("typename")
                .short('t')
                .long("type")
                .value_parser(clap::value_parser!(String))
                .required(true)
                .help("expecting a directory name"),
        )
        .subcommand(coresetcmd)
        .get_matches();
    //
    // retrieve HnswPathParams
    //
    let hdir = matches
        .get_one::<String>("dir")
        .expect("dir argument needed");
    let hname = matches
        .get_one::<String>("fname")
        .expect("hnsw base name needed");
    let tname: &String = matches
        .get_one::<String>("fname")
        .expect("typename required");
    //
    let hparams = HnswParams::new(hdir, hname, tname);
    //
    // parse coreset parameters
    //
    if let Some(core_match) = matches.subcommand_matches("coreset") {
        log::debug!("subcommand for coreset parameters");
        let res = parse_coreset_cmd(core_match);
        match res {
            Ok(params) => {
                core_params = params;
            }
            _ => {
                log::error!("parsing coreset command failed");
                println!("exiting with error {}", res.err().as_ref().unwrap());
                //  log::error!("exiting with error {}", res.err().unwrap());
                std::process::exit(1);
            }
        }
    } else {
        core_params = CoresetParams::default();
    }
    log::debug!("coreset params : {:?}", core_params);
    // retrieve
    //
    // Datamap Creation
    //
    let typename = "u32";
    let datamap = get_datamap(hparams.dir, hparams.hname, typename);
}