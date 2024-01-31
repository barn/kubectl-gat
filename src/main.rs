// use serde::{Deserialize, Serialize};
use chrono::DateTime;
use chrono::Utc;
use humantime::format_duration;
use regex::Regex;
use serde_json::Value;
use std::process::Command;

fn main() {
    let output = Command::new("kubectl")
        .arg("get")
        .arg("pods")
        .arg("-o=json")
        .output()
        .expect("failed to execute kubectl process");

    // println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("status: {}", output.status);
    println!("stderr: {}", String::from_utf8_lossy(&output.stderr));

    let v: Value = serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    // println!("some json: {}", v["items"]);

    if let Some(containers) = v["items"].as_array() {
        for n in containers {
            println!("");
            // println!("some json: {}", n);

            let mut containersready: i32 = 0;
            let containerscount = n["status"]["containerStatuses"].as_array().unwrap().len();

            for c in n["status"]["containerStatuses"].as_array().unwrap() {
                containersready += (c["ready"].as_bool() == Some(true)) as i32;
            }

            println!("containers: {}/{}", containersready, containerscount);

            // name
            let name = n["metadata"]["name"].as_str().unwrap();
            println!("name: {}", name);

            // status
            let status = n["status"]["phase"].as_str().unwrap();
            println!("status: {}", status);

            // restarts || pod['status']['containerStatuses'][0]['restartCount']
            let restartcount = n["status"]["containerStatuses"].as_array().unwrap()[0]
                ["restartCount"]
                .as_number()
                .unwrap();
            println!("restarts: {}", restartcount);

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
            println!("age: {}", s_replaced);
            // println!("age: {}", format_duration(diff.to_std().unwrap()));

            // image || "\n".join(list(map( lambda x: str(better_pods(x['image'])) , pod['spec']['containers'])))
            let mut images: Vec<String> = vec![];
            for c in n["spec"]["containers"].as_array().unwrap() {
                images.push(c["image"].as_str().unwrap().to_string());
            }
            if images.len() == 1 {
                println!("Image: {}", images[0]);
            } else {
                println!("Images: {:?}", images);
            }
        }
    }
}
