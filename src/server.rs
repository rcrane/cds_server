use iron;
use iron::prelude::*;
use iron::status;
use router::Router;

use serde_json;

use std::thread;
use std::fs::File;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::process::{Command, Stdio};

use std::io::{Write, Read};
use std::ops::DerefMut;

use super::{Result, ResultExt};

#[derive(Serialize, Deserialize, Debug)]
pub struct InvokeRequestBody {
    pub stdin: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct InvokeResponseBody {
    pub stdout: String,
    pub stderr: String,
    pub exit_status: i32,
    pub duration: u64,
    pub error: Option<String>,
}

pub struct Server {
    port: u16,
    config: Vec<(String, String)>,
}

// fn extract_config(json: serde_json::Value) -> Result<Vec<(String, String)>> {
//     let mut config = Vec::new();
//     match json.as_array() {
//         Some(arr) => {
//                 for i in 0..arr.len() {
//                     match arr[i].as_array() {
//                         Some(entry) => {
//                             ensure!(entry.len() == 2, "Entry {} of server config file doesn't contain two values!", i+1);
//                             ensure!(entry[0].is_string(), "First value of entry {} of server config file isn't a string!", i+1);
//                             ensure!(entry[1].is_string(), "Second value of entry {} of server config file isn't a string!", i+1);
//                             config.push((entry[0].as_string().unwrap_or("UNEXPECTED ERROR").to_owned(), entry[1].as_string().unwrap_or("UNEXPECTED_ERROR").to_owned()))
//                             //TODO: make sure the path leads to an executable
//                         }
//                         None => bail!("Entry {} of server config file isn't an array!", i+1)
//                 }
//             }
//         },
//         None => bail!("Server config file doesn't contain array!")
//     }
    
//     Ok(config)
// }

struct HumanHelpHandler {
    server: Arc<Server>,
}

impl iron::Handler for HumanHelpHandler {
    fn handle(&self, _: &mut Request) -> IronResult<Response> {
        let mut body = String::new();
        body.push_str("CDS Lab server\nThis server forwards request to native applications\nThe following programs are known by this server:\n");
        for entry in self.server.as_ref().config.iter() {
            body.push_str(entry.0.as_str());
            body.push_str("\n");
        }

        let mut resp = Response::new();
        resp.status = Some(status::Ok);
        resp.body = Some(Box::new(body));
        Ok(resp)
    }
}

struct InvokeHandler {
    server: Arc<Server>,
}

impl InvokeHandler {
    fn json_error(&self, error_msg: String) -> iron::IronResult<iron::Response> {
        let mut body: InvokeResponseBody = Default::default();
        body.error = Some(error_msg.clone());
        let response_body: String = match serde_json::to_string(&body) {
            Ok(s)  => s,
            Err(e) => {
                warn!("Error occurred during request handling, but unable to serialize error message ({}). Error: {:?}", error_msg, e);
                String::new()
            }
        };

        let error: super::Error = super::ErrorKind::Msg("".to_owned()).into();

        Err(IronError::new(Box::new(error), (iron::status::BadRequest, response_body)))
    }
}

impl iron::Handler for InvokeHandler {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let router = req.extensions.get::<Router>()
            .expect("internal error: InvokeHandler request has no Router extension!");
        let program = router
            .find("program")
            .expect("internal error: InvokerHandler request contains no :program param!");

        debug!("Received run request for program {}", program);

        debug!("Reading request's body");
        let mut req_body = String::new();
        if let Err(e) = req
            .body
            .read_to_string(&mut req_body)
        {
            return self.json_error(format!("Unable to read request's body! Error: {:?}", e));
        } 

        debug!("Parsing JSON");
        let req_body: InvokeRequestBody = match serde_json::from_str(req_body.as_str()) {
            Ok(b) => b,
            Err(e) => return self.json_error(format!("Unable to decode request's JSON body! Error: {:?}", e)),
        };

        debug!("Looking up location of {} ...", program);
        let location = match self.server
            .as_ref()
            .config.iter()
            .find(|ref x| x.0 == program)
        {
            Some(entry) => &entry.1,
            None        => {
                warn!("No such program in server configuration file!");
                let msg = format!("Unable to resolve location of program {}! No such entry in server configuration", program);
                return self.json_error(msg);
            }
        };

        debug!("Program is located at {}. Invoking program ...", location);
        let mut child = match Command::new(location)
                            .stdin(Stdio::piped())
                            .stdout(Stdio::piped())
                            .stderr(Stdio::piped())
                            .spawn()
        {
            Ok(child) => child,
            Err(e) => return self.json_error(format!("Unable to start program '{}' (location in container: {}). Error: {:?}", program, location, e))
        };

        debug!("Program started");
 
        // let child_id = child.id();
        // alternative: open /proc/:child_id/stat regularly parse for Zombie state, collect user, sys
        // issue with alternative: how to measure wall time?

        debug!("Decoding base64 encoded stdin message ...");
        let stdin_decoded = match base64::decode(req_body.stdin.as_str()) {
            Ok(decoded) => decoded,
            Err(e) => return self.json_error(format!("Unable to decode base64 stdin content. Error: {:?}", e))
        };

        debug!("Sending input to stdin ...");
        match child.stdin.take() {
            Some(mut stdin) => {
                if let Err(e) = stdin.write_all(stdin_decoded.as_slice()) {
                    return self.json_error(format!("Unable to write to stdin of invoked program. Error: {:?}", e))
                }
                if let Err(e) = stdin.flush() {
                    return self.json_error(format!("Unable to flush stdin of invoked program. Error: {:?}", e))
                }
                drop(stdin);
            },
            None => {
                if let Err(e) = child.kill() {
                    warn!("Unable to kill child process after allocation of stdin stream failed! Error: {:?}", e);
                }
                return self.json_error("Unable to open stdin of invoked program".to_owned());
            }
        };
        debug!("Starting timer ...");
        let start = Instant::now();

        let mut stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                return self.json_error(format!("Unable to obtain stdout of invoked program"));
            }
        };

        debug!("Spawning stdout & stderr collector threads");
        let stdout_str = Arc::new(Mutex::new(String::new()));
        let stdout_str2 = stdout_str.clone();
        let stdout_collector = thread::spawn(move || {
            match stdout_str2.lock() {
                Ok(mut stdout_guard) => {
                    if let Err(e) = stdout.read_to_string(stdout_guard.deref_mut()) {
                        warn!("Unable to read stdout of program: {:?}", e);
                    }
                },
                Err(e) => warn!("Unable to lock stdout collector stream: {:?}", e)
            };
        });


        let stderr_str = Arc::new(Mutex::new(String::new()));
        let stderr_str2 = stderr_str.clone();
        let stderr_collector = match child.stderr.take() {
            Some(mut stderr) => {
                Some(thread::spawn(move || {
                    match stderr_str2.lock() {
                        Ok(mut stderr_guard) => match stderr.read_to_string(stderr_guard.deref_mut()) {
                            Err(e) => warn!("Unable to read stderr of program: {:?}", e),
                            _ => {},
                        },
                        Err(e) => warn!("Unable to lock stderr collector stream: {:?}", e)
                    };
                }))
            },
            None => {
                warn!("Unable to obtain program's stderr!");
                None
            },
        };

        debug!("Waiting for program to terminate ...");
        let exit_status = match child.wait() {
            Ok(s) => s,
            Err(e) => return self.json_error(format!("An error occured while waiting for child program to finish. Error: {:?}", e))
        };
        let duration = start.elapsed();

        debug!("Program terminated. Joining collector threads ...");
        if let Err(e) = stdout_collector.join() {
            warn!("Unable to join stdout collector thread: {:?}", e);
        }
        if let Some(t) = stderr_collector {
            if let Err(e) = t.join() {
                warn!("Unable to join stderr collector thread: {:?}", e);
            }
        };

        debug!("Collector threads joined. Generating response ...");

        let mut resp_body: InvokeResponseBody = Default::default();
        resp_body.exit_status = exit_status.code().unwrap_or(-1);
        resp_body.duration = duration.as_secs() * 1000 * 1000 + duration.subsec_nanos() as u64 / 1000;

        if let Ok(stdout) = stdout_str.lock() {
            resp_body.stdout = base64::encode(std::convert::AsRef::<[u8]>::as_ref(stdout.as_str()));
        };
        if let Ok(stderr) = stderr_str.lock() {
            resp_body.stderr = base64::encode(std::convert::AsRef::<[u8]>::as_ref(stderr.as_str()));
        }

        Ok(Response::with((status::Ok, serde_json::to_string(&resp_body).unwrap())))
    }
}


impl Server {
    pub fn new(config_path: &Path, port: u16) -> Result<Server> {
        let err_msg = format!("Unable to interpret server config file {:?}", config_path);
        let mut config_file = File::open(config_path)
                                       .chain_err(|| err_msg.clone())?;

        // let config_json = Json::from_reader(&mut config_file)
        //                                .chain_err(|| err_msg)?;

        // let config = extract_config(config_json)
        //                                .chain_err(|| err_msg)?;

        let config = serde_json::from_reader(&mut config_file)
                                       .chain_err(|| err_msg.clone())?;

        Ok(Server{port: port, config: config})
    }

    pub fn run(self) -> Result<Arc<Server>> {
        let arc = Arc::new(self);

        let mut router = Router::new();
        router.post("/run/:program", InvokeHandler{server: arc.clone()} , "invoke");
        router.any("/*", HumanHelpHandler{server: arc.clone()}, "human");

        Iron::new(router)
            .http(("0.0.0.0", arc.as_ref().port))
            .chain_err(|| "Unable to start server!")?;

        Ok(arc)
    }

}
