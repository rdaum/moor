extern crate core;

use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::net::SocketAddr;
use std::sync::Arc;

use clap::builder::ValueHint;
use clap::Parser;
use clap_derive::Parser;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Error;

use crate::compiler::codegen::compile;
use crate::db::inmem_db::ImDB;
use crate::db::inmem_db_worldstate::ImDbWorldStateSource;
use crate::model::objects::{ObjAttrs, ObjFlag};
use crate::model::props::PropFlag;
use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
use crate::model::var::{Objid, Var};
use crate::model::verbs::VerbFlag;
use crate::server::scheduler::Scheduler;
use crate::textdump::{Object, TextdumpReader};
use crate::util::bitenum::BitEnum;

pub mod compiler;
pub mod db;
pub mod grammar;
pub mod model;
pub mod server;
pub mod textdump;
pub mod util;
pub mod vm;

struct RProp {
    definer: Objid,
    name: String,
    owner: Objid,
    flags: u8,
    val: Var,
}

fn resolve_prop(omap: &HashMap<Objid, Object>, offset: usize, o: &Object) -> Option<RProp> {
    let local_len = o.propdefs.len();
    if offset < local_len {
        let name = o.propdefs[offset].clone();
        let pval = &o.propvals[offset];
        return Some(RProp {
            definer: o.id,
            name,
            owner: pval.owner,
            flags: pval.flags,
            val: pval.value.clone(),
        });
    }

    let offset = offset - local_len;

    let parent = omap.get(&o.parent)?;
    resolve_prop(omap, offset, parent)
}

const VF_READ: u16 = 1;
const VF_WRITE: u16 = 2;
const VF_EXEC: u16 = 4;
const VF_DEBUG: u16 = 10;
const VF_PERMMASK: u16 = 0xf;
const VF_DOBJSHIFT: u16 = 4;
const VF_IOBJSHIFT: u16 = 4;
const VF_OBJMASK: u16 = 0x3;

const VF_ASPEC_NONE: u16 = 0;
const VF_ASPEC_ANY: u16 = 1;
const VF_ASPEC_THIS: u16 = 2;

const PREP_ANY: i16 = -2;
const PREP_NONE: i16 = -1;

fn cv_prep_flag(vprep: i16) -> PrepSpec {
    match vprep {
        PREP_ANY => PrepSpec::Any,
        PREP_NONE => PrepSpec::None,
        _ => PrepSpec::Other(vprep as u16),
    }
}

fn cv_aspec_flag(flags: u16) -> ArgSpec {
    match flags {
        VF_ASPEC_NONE => ArgSpec::None,
        VF_ASPEC_ANY => ArgSpec::Any,
        VF_ASPEC_THIS => ArgSpec::This,
        _ => panic!("Unsupported argpsec"),
    }
}

fn textdump_load(s: &mut ImDB, path: &str) -> Result<(), anyhow::Error> {
    let jhcore = File::open(path)?;
    let br = BufReader::new(jhcore);
    let mut tdr = TextdumpReader::new(br);
    let td = tdr.read_textdump()?;

    let mut tx = s.do_begin_tx().expect("Unable to start transaction");

    // Pass 1 Create objects
    eprintln!("Instantiating objects...");
    for (objid, o) in &td.objects {
        let flags: BitEnum<ObjFlag> = BitEnum::from_u8(o.flags);

        s.create_object(
            &mut tx,
            Some(*objid),
            ObjAttrs::new()
                .owner(o.owner)
                .location(o.location)
                .name(o.name.as_str())
                .parent(o.parent)
                .flags(flags),
        )?;
    }
    eprintln!("Instantiated objects\nDefining props...");

    // Pass 2 define props
    for (objid, o) in &td.objects {
        for (pnum, _) in o.propvals.iter().enumerate() {
            let resolved = resolve_prop(&td.objects, pnum, o).unwrap();
            let flags: BitEnum<PropFlag> = BitEnum::from_u8(resolved.flags);
            if resolved.definer == *objid {
                eprintln!("Defining prop: #{}.{}", objid.0, resolved.name);
                let res = s.add_propdef(
                    &mut tx,
                    *objid,
                    resolved.name.as_str(),
                    resolved.owner,
                    flags,
                    None,
                );
                if res.is_err() {
                    eprintln!(
                        "Unable to define property {}.{}: {:?}",
                        objid.0, resolved.name, res
                    );
                }
            }
        }
    }

    eprintln!("Defined props\nSetting props...");
    // Pass 3 set props
    for (objid, o) in &td.objects {
        for (pnum, p) in o.propvals.iter().enumerate() {
            let resolved = resolve_prop(&td.objects, pnum, o).unwrap();
            let flags: BitEnum<PropFlag> = BitEnum::from_u8(resolved.flags);
            eprintln!("Setting prop: #{}.{}", objid.0, resolved.name);

            let result = s.get_propdef(&mut tx, resolved.definer, resolved.name.as_str());
            let Ok(pdf) = result else {
                eprintln!("Unable to find property {}.{}: {:?}", objid.0, resolved.name, result);
                continue;
            };
            let result = s.set_property(
                &mut tx,
                pdf.pid,
                *objid,
                p.value.clone(),
                resolved.owner,
                flags,
            );
            if result.is_err() {
                eprintln!(
                    "Unable to set property {}.{}: {:?}",
                    objid.0, resolved.name, result
                );
            }
        }
    }

    eprintln!("Set props\nDefining verbs...");
    // Pass 4 define verbs
    for (objid, o) in &td.objects {
        for (vn, v) in o.verbdefs.iter().enumerate() {
            let mut flags: BitEnum<VerbFlag> = BitEnum::new();
            let permflags = v.flags & VF_PERMMASK;
            if permflags & VF_READ != 0 {
                flags |= VerbFlag::Read;
            }
            if permflags & VF_WRITE != 0 {
                flags |= VerbFlag::Write;
            }
            if permflags & VF_EXEC != 0 {
                flags |= VerbFlag::Exec;
            }
            let dobjflags = (v.flags >> VF_DOBJSHIFT) & VF_OBJMASK;
            let iobjflags = (v.flags >> VF_DOBJSHIFT) & VF_OBJMASK;

            let argspec = VerbArgsSpec {
                dobj: cv_aspec_flag(dobjflags),
                prep: cv_prep_flag(v.prep),
                iobj: cv_aspec_flag(iobjflags),
            };

            let names: Vec<&str> = v.name.split(' ').collect();

            let verb = match td.verbs.get(&(*objid, vn)) {
                None => {
                    println!("Could not find verb #{}/{} ({:?}", objid.0, vn, names);
                    continue;
                }
                Some(v) => v,
            };

            eprintln!(
                "Compiling #{}/{} (#{}:{})...",
                objid.0, names[0], objid.0, vn
            );

            let binary = match compile(verb.program.as_str()) {
                Ok(b) => b,
                Err(e) => {
                    panic!("Compile error in #{}/{}: {:?}", objid.0, names[0], e);
                }
            };

            let av = s.add_verb(
                &mut tx,
                *objid,
                names.clone(),
                v.owner,
                flags,
                argspec,
                binary,
            );
            if av.is_err() {
                eprintln!(
                    "Unable to add verb: #{}:{}: {:?}",
                    objid.0,
                    names.first().unwrap(),
                    av
                );
            }
        }
    }
    eprintln!("Verbs defined.\nImport complete.");

    s.do_commit_tx(&mut tx)?;

    Ok(())
}

#[derive(Parser, Debug)] // requires `derive` feature
struct Args {
    #[arg(value_name = "db", help = "Path to database file to use or create", value_hint = ValueHint::FilePath)]
    db: std::path::PathBuf,

    #[arg(value_name = "textdump", help = "Path to textdump to import", value_hint = ValueHint::FilePath)]
    textdump: Option<std::path::PathBuf>,

    #[arg(value_name = "listen", help = "Listen address")]
    listen_address: Option<String>,
}

async fn accept_connection(scheduler: Arc<Mutex<Scheduler>>, peer: SocketAddr, stream: TcpStream) {
    if let Err(e) = handle_connection(scheduler.clone(), peer, stream).await {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => eprintln!("Error processing connection: {}", err),
        }
    }
}

async fn handle_connection(
    scheduler: Arc<Mutex<Scheduler>>,
    peer: SocketAddr,
    stream: TcpStream,
) -> Result<(), tungstenite::Error> {
    let mut ws_stream = accept_async(stream).await.expect("Failed to accept");

    eprintln!("New WebSocket connection: {}", peer);

    while let Some(msg) = ws_stream.next().await {
        let msg = msg?;
        if msg.is_text() || msg.is_binary() {
            let mut scheduler = scheduler.lock().await;
            let cmd = msg.into_text().unwrap();
            let cmd = cmd.as_str().trim();
            let setup_result = scheduler.setup_parse_command_task(Objid(2), cmd).await;
            let Ok(task_id) = setup_result else {
                eprintln!("Unable to parse command ({}): {:?}", cmd, setup_result);
                ws_stream.send(format!("Unable to parse command ({}): {:?}", cmd, setup_result).into()).await?;

                continue;
            };
            eprintln!("Task: {:?}", task_id);

            if let Err(e) = scheduler.start_task(task_id).await {
                eprintln!("Unable to execute: {}", e);
                ws_stream
                    .send(format!("Unable to execute: {}", e).into())
                    .await?;

                continue;
            };
            ws_stream
                .send(format!("Command parsed correctly and ran in task {:?}", task_id).into())
                .await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    let args: Args = Args::parse();

    eprintln!("Moor");

    let mut src = ImDB::new();
    if let Some(textdump) = args.textdump {
        eprintln!("Loading textdump...");
        textdump_load(&mut src, textdump.to_str().unwrap()).unwrap();
    }

    let mut tx = src.do_begin_tx().unwrap();

    // Move wizard (#2) into first room (#70) for purpose of testing, so that there's something to
    // match against.
    src.object_set_attrs(
        &mut tx,
        Objid(2),
        ObjAttrs {
            owner: None,
            name: None,
            parent: None,
            location: Some(Objid(70)),
            flags: None,
        },
    )
    .unwrap();
    src.do_commit_tx(&mut tx).unwrap();

    let state_src = Arc::new(Mutex::new(ImDbWorldStateSource::new(src)));
    let scheduler = Arc::new(Mutex::new(Scheduler::new(state_src.clone())));

    let addr = args
        .listen_address
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");
    eprintln!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream
            .peer_addr()
            .expect("connected streams should have a peer address");

        tokio::spawn(accept_connection(scheduler.clone(), peer, stream));
    }

    eprintln!("Done.");

    Ok(())
}
