// use serde::{Deserialize, Serialize};
use chrono::DateTime;
use chrono::Utc;
use clap::{Parser, Subcommand};
use humantime::format_duration;
use regex::Regex;
use serde_json::Value;
use std::process;
use std::process::Command;
use tabled::settings::{object::Columns, object::Rows, Disable, Padding, Style, Theme};
use tabled::{Table, Tabled};

// Pascal is "Keep all letters uppercase and indicate word boundaries with underscores."
#[derive(Tabled)]
#[tabled(rename_all = "Pascal")]
struct Pod {
    name: String,
    #[tabled(rename = "Ready")]
    containerstatus: String,
    status: String,
    restarts: i32,
    age: String,
    images: String,
}

fn build_pod(
    name: String,
    containerstatus: String,
    status: String,
    restarts: i32,
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

    // println!("args {:?}", std::env::args());
    // println!("looking for {}", findmepls);
    // process::exit(0);
    //

    // we have a whole subcommand mess, and then just call 'get pods' anyway
    let mut arr = vec!["get", "pods", "-o=json"];
    let mut ns = String::new();
    if !namespacetouse.is_empty() {
        ns.push_str(format!("--namespace={}", namespacetouse).as_str());
        arr.push(&ns);
    }

    // let output = Command::new("kubectl")
    //     .arg("get")
    //     .arg("pods")
    //     .arg("-o=json")
    //     .output()
    //     .expect("failed to execute kubectl process");
    let output = Command::new("kubectl")
        .args(arr)
        .output()
        .expect("failed to execute kubectl process");

    // maybe check output.status?
    if !output.status.success() {
        match output.status.code() {
            Some(code) => {
                println!("failed to kubectl with {}", output.status);
                process::exit(code);
            }
            None => {}
        }
    }

    // println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    // println!("status: {}", output.status);
    // println!("stderr: {}", String::from_utf8_lossy(&output.stderr));

    let v: Value = serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    // println!("some json: {}", v["items"]);

    if let Some(containers) = v["items"].as_array() {
        for n in containers {
            // println!("");
            // println!("some json: {}", n);

            let mut containersready: i32 = 0;
            let containerscount = n["status"]["containerStatuses"].as_array().unwrap().len() as i32;

            for c in n["status"]["containerStatuses"].as_array().unwrap() {
                containersready += (c["ready"].as_bool() == Some(true)) as i32;
            }

            // println!("containers: {}/{}", containersready, containerscount);

            // name
            let name = n["metadata"]["name"].as_str().unwrap();
            // println!("name: {}", name);
            //
            if !findmepls.is_empty() {
                let findmere = Regex::new(format!(r"{}", findmepls).as_str()).unwrap();
                if !findmere.is_match(name) {
                    continue;
                }
            }

            // status
            let status = n["status"]["phase"].as_str().unwrap();
            // println!("status: {}", status);

            // restarts || pod['status']['containerStatuses'][0]['restartCount']
            let restartcount = n["status"]["containerStatuses"].as_array().unwrap()[0]
                ["restartCount"]
                .as_i64()
                .unwrap() as i32;
            // println!("restarts: {}", restartcount);

            // age || datetime.now(timezone.utc) - datetime.fromisoformat(pod['metadata']['creationTimestamp'])
            let creationtime = n["metadata"]["creationTimestamp"].as_str().unwrap();
            let datetime: DateTime<Utc> = creationtime.parse().unwrap();
            let end_time = chrono::offset::Utc::now();

            let diff = end_time - datetime;

            // do ugly string parsing on the string of the time delta, rather than doing it
            // properly
            let re = Regex::new(r" \d+[mu]s").unwrap();
            let time_diff_str = format_duration(diff.to_std().unwrap()).to_string();
            let s_replaced = re.replace_all(&time_diff_str, "");
            // println!("age: {}", s_replaced);
            // println!("age: {}", format_duration(diff.to_std().unwrap()));

            // image || "\n".join(list(map( lambda x: str(better_pods(x['image'])) , pod['spec']['containers'])))
            let mut images: Vec<String> = vec![];
            let ree = Regex::new(r"301643779712\.dkr\.ecr\.us-east-1\.amazonaws\.com").unwrap();
            for c in n["spec"]["containers"].as_array().unwrap() {
                let shorter_image = ree.replace_all(c["image"].as_str().unwrap(), "<clr-ecr>");
                images.push(shorter_image.to_string());
            }
            // if images.len() == 1 {
            //     println!("Image: {}", images[0]);
            // } else {
            //     println!("Images: {:?}", images);
            // }

            // make containerscount and ready to a string of X/Y
            let containerstatus = format!("{}/{}", containersready, containerscount);

            tidepods.push(build_pod(
                name.to_string(),
                containerstatus,
                status.to_string(),
                restartcount,
                s_replaced.to_string(),
                images[0].to_string(),
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

    if args.no_headers == true {
        ourtable.with(Disable::row(Rows::first()));
    }

    println!("{}", ourtable);
}
