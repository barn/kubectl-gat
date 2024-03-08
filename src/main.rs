use chrono::DateTime;
use chrono::Utc;
use clap::{Parser, Subcommand};
use humantime::format_duration;
use itertools::Itertools;
use regex::Regex;
use serde_json::Map;
use serde_json::Value;
use std::collections::HashMap;
// use std::collections::HashSet;
use config::{Config, File};
use std::process;
use std::process::Command;
use tabled::settings::{object::Columns, object::Rows, Disable, Padding, Style, Theme};
use tabled::{Table, Tabled};
extern crate dirs;

// Pascal is "Keep all letters uppercase and indicate word boundaries with underscores."
#[derive(Tabled)]
#[tabled(rename_all = "Pascal")]
struct Pod {
    name: String,
    #[tabled(rename = "Ready")]
    containerstatus: String,
    status: String,
    restarts: i64,
    age: String,
    securitybits: String,
    images: String,
}

fn build_pod(
    name: String,
    containerstatus: String,
    status: String,
    restarts: i64,
    age: String,
    securitybits: String,
    images: String,
) -> Pod {
    Pod {
        name,
        containerstatus,
        status,
        restarts,
        age,
        securitybits,
        images,
    }
}

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(short, long)]
    no_headers: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Adds files to myapp
    Pods {
        name: Option<String>,
        #[arg(short, long)]
        namespace: Option<String>,
    },
}

pub struct PodSecurityContext {
    user: i64,
    group: i64,
    fsgroup: i64,
    runasnonroot: bool,
    seccomp: bool,
    seccomp_runtimedef: bool,
}

#[derive(Debug)]
pub struct ContainerSecurityContext {
    allowpriv: bool,
    dropping: bool,
    readonlyfs: bool,
    drops: Vec<String>,
    adds: Vec<String>,
}

impl ContainerSecurityContext {
    pub fn new(someinput: &Map<String, Value>) -> ContainerSecurityContext {
        let mut has_allow_privesc: bool = true;
        let mut has_dropping: bool = false;
        let mut caps_we_drop: Vec<String> = vec![];
        let mut caps_we_add: Vec<String> = vec![];

        // if let Some(t) = c["securityContext"].as_object() {
        //     println!("no idea {:?}", t);
        if someinput.contains_key("allowPrivilegeEscalation") {
            if let Some(t) = someinput["allowPrivilegeEscalation"].as_bool() {
                has_allow_privesc = t;
            }
        }

        if someinput.contains_key("capabilities") {
            if let Some(t) = someinput["capabilities"].as_object() {
                has_dropping = true;
                // println!("we got {:?}", t);

                if t.contains_key("drop") {
                    if let Some(kdrops) = t["drop"].as_array() {
                        for k in kdrops {
                            if let Some(something) = k.as_str() {
                                caps_we_drop.push(something.to_string());
                            }
                        }
                    }
                }
                if t.contains_key("add") {
                    if let Some(kadds) = t["add"].as_array() {
                        for k in kadds {
                            if let Some(something) = k.as_str() {
                                caps_we_add.push(something.to_string());
                            }
                        }
                    }
                }
            }
        }

        ContainerSecurityContext {
            allowpriv: has_allow_privesc,
            readonlyfs: false,
            dropping: has_dropping,
            drops: caps_we_drop,
            adds: caps_we_add,
        }
    }
    pub fn dropping_all(&self) -> bool {
        self.dropping && self.drops.len() == 1 && self.adds.is_empty() && self.drops[0] == "ALL"
    }
    pub fn getsecbits(&self) -> String {
        let mut output = String::new();

        if !self.allowpriv {
            output.push('🔐')
        } else {
            output.push_str("␛ ")
        };

        if self.dropping_all() {
            output.push('🫳');
        }

        output
    }
}

impl std::fmt::Display for ContainerSecurityContext {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            fmt,
            "allowpriv {}, readonlyfs {}, is dropping {}.\n    dropping: {:?}/adding: {:?}",
            self.allowpriv, self.readonlyfs, self.dropping, self.drops, self.adds
        )
    }
}
impl std::fmt::Display for PodSecurityContext {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "My user is {}.", self.printuid())
    }
}

impl PodSecurityContext {
    pub fn new(someinput: &Map<String, Value>) -> PodSecurityContext {
        let mut seccomp = false;
        let mut seccomp_runtimedef = false;
        let mut kvs = HashMap::new();
        let mut runasnonroot: bool = false;

        for key in ["runAsUser", "fsGroup", "runAsGroup"] {
            kvs.insert(key, 0);
            if someinput.contains_key(key) {
                if let Some(value) = someinput[key].as_i64() {
                    kvs.insert(key, value);
                }
            }
        }

        if someinput.contains_key("seccompProfile") {
            seccomp = true;
            let seccy = someinput["seccompProfile"].as_object().unwrap();
            // println!("seccomp is {:?}", seccy);
            if seccy.contains_key("type") && seccy["type"].as_str().unwrap() == "RuntimeDefault" {
                seccomp_runtimedef = true;
            }
        }

        if someinput.contains_key("runAsNonRoot") {
            if let Some(value) = someinput["runAsNonRoot"].as_bool() {
                runasnonroot = value;
            }
        }

        PodSecurityContext {
            user: kvs.get("runAsUser").copied().unwrap_or(0),
            group: kvs.get("runAsGroup").copied().unwrap_or(0),
            fsgroup: kvs.get("fsGroup").copied().unwrap_or(0),
            runasnonroot,
            seccomp,
            seccomp_runtimedef,
        }
    }

    fn sameuid(&self) -> bool {
        self.user == self.group && self.group == self.fsgroup && self.user == self.fsgroup
    }

    pub fn printuid(&self) -> String {
        if self.sameuid() {
            self.user.to_string()
        } else {
            format!("{}/{}/{}", self.user, self.group, self.fsgroup)
        }
    }

    pub fn getsecbits(&self) -> String {
        let mut output = String::new();

        if self.runasnonroot {
            output.push_str("R✅");
        } else {
            output.push_str("R😭");
        }

        if self.seccomp {
            output.push('👮');
            if self.seccomp_runtimedef {
                output.push_str("⚙️");
            }
            output.push(' ');
        }

        output.push_str(&self.printuid());

        output
    }
}

fn main() {
    let mut tidepods: Vec<Pod> = vec![];
    let mut ecr_renames: HashMap<String, String> = HashMap::new();

    let args = Cli::parse();

    // println!(
    //     "pattern: {:?}: dearth of headers? {:?}",
    //     args.command, args.no_headers
    // );

    let mut findmepls = String::new();
    let mut namespacetouse = String::new();
    match &args.command {
        Commands::Pods { name, namespace } => {
            match name {
                Some(x) => findmepls = x.to_string(),
                &None => {}
            }
            match namespace {
                Some(x) => namespacetouse = x.to_string(),
                &None => {}
            }
            // println!("'kubectl get pods' was used, name is: {}", name);
        }
    }

    let filename = dirs::home_dir()
        .unwrap()
        .join(".config")
        .join("kubectlgat.toml");
    if filename.exists() {
        let settingz = Config::builder()
            .add_source(File::from(filename))
            .build()
            .unwrap();

        // for (key, val) in settingz.get_table("renames").unwrap().iter() {
        //     ecr_renames.insert(key.clone(), val.to_string());
        //     // println!("we have a {} and a {}", key, val.to_string());
        // }
        settingz
            .get_table("renames")
            .unwrap_or(HashMap::new())
            .iter()
            .for_each(|(k, v)| {
                ecr_renames.insert(k.clone(), v.to_string());
            });
    }

    // we have a whole subcommand mess, and then just call 'get pods' anyway
    let mut arr = vec!["get", "pods", "-o=json"];
    let mut ns = String::new();
    if !namespacetouse.is_empty() {
        ns.push_str(format!("--namespace={}", namespacetouse).as_str());
        arr.push(&ns);
    }

    let output = Command::new("kubectl")
        .args(arr)
        .output()
        .expect("failed to execute kubectl process");

    // maybe check output.status?
    if !output.status.success() {
        // match output.status.code() {
        //     Some(code) => {
        if let Some(code) = output.status.code() {
            println!("failed to kubectl with {}", output.status);
            process::exit(code);
        }
        // }
        // None => {}
        // }
    }

    let v: Value = serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();

    if let Some(containers) = v["items"].as_array() {
        for n in containers {
            // println!("");
            // println!("some json: {}", n);

            // we reference containerstatuses so many times, it makes sense just to carve this off
            // and go with it.
            let cs = n["status"]["containerStatuses"]
                .as_array()
                .expect("Didnt get a containerStatus");

            let mut containersready: i32 = 0;
            let containerscount = cs.len() as i32;

            for c in cs {
                containersready += (c["ready"].as_bool() == Some(true)) as i32;
            }

            // name
            let name = n["metadata"]["name"].as_str().unwrap();
            // println!("name: {}", name);

            // do we want to filter names?
            if !findmepls.is_empty() {
                let findmere = Regex::new(findmepls.to_string().as_str()).unwrap();
                if !findmere.is_match(name) {
                    continue;
                }
            }

            // status
            let mut status: String = n["status"]["phase"].as_str().unwrap().to_string();

            if status != "Running" {
                let mut statuses: Vec<String> = vec![];
                for c in cs {
                    if let Some(value) = c["state"]["waiting"]["reason"].as_str() {
                        statuses.push(value.to_string())
                    }
                }
                if !statuses.is_empty() {
                    status = statuses
                        .clone()
                        .into_iter()
                        .unique()
                        .collect::<Vec<_>>()
                        .join(", ")
                        .to_string();
                }
            }

            // restarts || pod['status']['containerStatuses'][0]['restartCount']
            let restartcount: i64 = cs
                .iter()
                .map(|x| x["restartCount"].as_i64().unwrap())
                .collect::<Vec<i64>>()
                .iter()
                .sum::<i64>();

            let creationtime = n["metadata"]["creationTimestamp"].as_str().unwrap();
            let datetime: DateTime<Utc> = creationtime.parse().unwrap();
            let diff = chrono::offset::Utc::now() - datetime;

            // do ugly string parsing on the string of the time delta, rather than doing it
            // properly
            let re = Regex::new(r" \d+[mu]s").unwrap();
            let time_diff_str = format_duration(diff.to_std().unwrap()).to_string();
            let s_replaced = re.replace_all(&time_diff_str, "");

            // image || "\n".join(list(map( lambda x: str(better_pods(x['image'])) , pod['spec']['containers'])))
            let mut images: Vec<String> = vec![];

            let mut security_bits = String::new();
            for c in n["spec"]["containers"].as_array().unwrap() {
                let curimagestring = c["image"].as_str().unwrap();
                let mut newimagestring = curimagestring.to_string();

                for (big, smol) in &ecr_renames {
                    let ree = Regex::new(big).unwrap();
                    let shorter_image = ree.replace_all(curimagestring, format!("<{}>", smol));
                    newimagestring = shorter_image.to_string();
                    if newimagestring != curimagestring {
                        break;
                    }
                }
                images.push(newimagestring);

                if let Some(t) = c["securityContext"].as_object() {
                    let xxyy = ContainerSecurityContext::new(t);
                    security_bits.push_str(&xxyy.getsecbits());
                    break;
                    // if xxyy.dropping_all() {
                    //     println!("🎉");
                    // } else {
                    //     println!("boooooo");
                    // }
                    // println!("printy print {}", xxyy);
                }
            }

            let sec = n["spec"]["securityContext"].as_object().unwrap();
            // if !sec.is_empty() {
            //     println!("securities for {} are {:?}", name, sec);
            // }
            // mangle_security_contexts(sec.clone());
            let xxx = PodSecurityContext::new(sec);
            security_bits.push_str(&xxx.getsecbits().to_string());

            // we could return them all? but returning a unique list is going to save space. And
            // this is meant to be as similar to `get pods` as possible
            let mut images_sorted: Vec<String> = images.clone();
            images_sorted.sort_unstable();
            images_sorted.dedup();

            // make containerscount and ready to a string of X/Y
            let containerstatus = format!("{}/{}", containersready, containerscount);

            tidepods.push(build_pod(
                name.to_string(),
                containerstatus,
                status.to_string(),
                restartcount,
                s_replaced.to_string(),
                security_bits,
                images_sorted.join("\n"),
            ))
        }
    }

    // https://github.com/zhiburt/tabled/blob/master/README.md#theme
    let mut table = Table::new(&tidepods);
    let mut style = Theme::from_style(Style::empty());
    style.remove_border_horizontal();
    style.remove_border_vertical();

    // style.align_columns(Alignment::left());
    //
    let ourtable = table
        .with(style)
        .modify(Columns::first(), Padding::new(0, 0, 0, 0));

    if args.no_headers {
        ourtable.with(Disable::row(Rows::first()));
    }

    println!("{}", ourtable);
}
