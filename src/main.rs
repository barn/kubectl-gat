// use serde::{Deserialize, Serialize};
use chrono::DateTime;
use chrono::Utc;
use clap::{Parser, Subcommand};
use humantime::format_duration;
use itertools::Itertools;
use regex::Regex;
use serde_json::Value;
use std::process;
use std::process::Command;
use tabled::settings::{object::Columns, object::Rows, Disable, Padding, Style, Theme};
use tabled::{Table, Tabled};

const WORK_ECR: &str = "***REMOVED***";
const WORK_ECR_SHORT: &str = "<clr-ecr>";

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
    images: String,
}

fn build_pod(
    name: String,
    containerstatus: String,
    status: String,
    restarts: i64,
    age: String,
    images: String,
) -> Pod {
    Pod {
        name,
        containerstatus,
        status,
        restarts,
        age,
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

fn main() {
    let mut tidepods: Vec<Pod> = vec![];

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
            let ree = Regex::new(WORK_ECR).unwrap();
            for c in n["spec"]["containers"].as_array().unwrap() {
                let shorter_image = ree.replace_all(c["image"].as_str().unwrap(), WORK_ECR_SHORT);
                images.push(shorter_image.to_string());
            }

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
