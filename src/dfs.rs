use serde_json::json;
use serde::{Deserialize};
use reqwest::multipart::{Part, Form};
use tokio::fs::File as TFile;
use std::fs::File;
use std::io::Write;
use std::error::Error;
use futures_util::StreamExt;

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Config {
    pub endpoint: String,
    pub username: String,
    pub password: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct SharedPod {
    pub podSharingReference: String,
}

#[derive(Debug, Deserialize)]
pub struct ResponseMessage {
    pub message: String,
}

pub async fn login(
    dfs_client: &reqwest::Client,
    config: &Config,
) -> Result<String, Box<dyn Error>> {
    let req_body = json!({
        "username": config.username,
        "password": config.password,
    });
    println!("{:#?}", config);
    let cookie = dfs_client.post(
            format!("{}/v2/user/login", config.endpoint)
        )
        .header("Content-Type", "application/json")
        .json(&req_body)
        .send()  
        .await?
        .headers()
        .get("set-cookie")
        .ok_or_else(|| "Header should contain `set-cookie` field.")?
        .to_str()?
        .to_string();
    Ok(cookie)
}

pub async fn new_pod(
    dfs_client: &reqwest::Client,
    config: &Config,
    cookie: &String,
    pod_name: String,
) -> Result<(), Box<dyn Error>> {
    println!("New pod request: `{}`", pod_name);
    let response = dfs_client.post(
            format!("{}/v1/pod/new", config.endpoint)
        )
        .header("Content-Type", "application/json")
        .header("Cookie", cookie)
        .json(&json!({
            "podName": pod_name,
            "password": config.password,
        }))        
        .send()
        .await?;
    if false == response.status().is_success() {        
        return Err(format!("New pod error: `{:?}`",
            response.json::<ResponseMessage>().await?.message).as_str().into()
        );
    }    

    println!("Pod creation was succeessful.");
    Ok(())   
}

pub async fn open_pod(
    dfs_client: &reqwest::Client,
    config: &Config,
    cookie: &String,
    pod_name: String,
) -> Result<(), Box<dyn Error>> {
    println!("Open pod request for: `{}`", pod_name);
    let response = dfs_client.post(
            format!("{}/v1/pod/open", config.endpoint)
        )
        .header("Cookie", cookie)
        .header("Content-Type", "application/json")
        .json(&json!({
            "podName": pod_name,
            "password": config.password,
        }))
        .send()
        .await?;
    if false == response.status().is_success() {        
        return Err(format!("Open pod error: `{:?}`",
            response.json::<ResponseMessage>().await?.message).as_str().into()
        );
    }   
    println!("Pod open was successful."); 
    Ok(())
}

pub async fn share_pod(
    dfs_client: &reqwest::Client,
    config: &Config,
    cookie: &String,
    pod_name: String,
) -> Result<SharedPod, Box<dyn Error>> {
    println!("Share pod request for: `{}`", pod_name);
    let response = dfs_client.post(
            format!("{}/v1/pod/share", config.endpoint)
        )
        .header("Cookie", cookie)
        .header("Content-Type", "application/json")
        .json(&json!({
            "podName": pod_name,
            "password": config.password,
        }))
        .send()
        .await?;
        
    if false == response.status().is_success() {        
        return Err(format!("Share pod error: `{:?}`",
            response.json::<ResponseMessage>().await?.message).as_str().into()
        );
    }  

    println!("Pod sharing was successful.");
    Ok(response.json::<SharedPod>().await?)
}

pub async fn fork_pod(
    dfs_client: &reqwest::Client,
    config: &Config,
    cookie: &String,
    sharing_ref: String,
) -> Result<(), Box<dyn Error>> {
    println!("fork pod request, cid `{}`",
        sharing_ref);
    let response = dfs_client.get(
            format!("{}/v1/pod/receive", config.endpoint)
        )
        .header("Cookie", cookie)
        .header("Content-Type", "application/json")
        // .json(&json!({
        //     "sharingRef": sharing_ref,
        // }))
        .query(&[("sharingRef", sharing_ref)])
        .send()
        .await?;
    if false == response.status().is_success() {        
        return Err(format!("Fork pod error: `{:?}`",
            response.json::<ResponseMessage>().await?.message).as_str().into()
        );
    }   

    println!("Fork pod was successful.");
    Ok(())    
}

pub async fn download_file(
    dfs_client: &reqwest::Client,
    config: &Config,
    cookie: &String,
    pod_name: String,
    from: String,        
    to: String           
) -> Result<(), Box<dyn Error>> {
    // download a file from a pod and save it to the local path
    println!("File download request, from: `{}:{}`, to: `{}`",
        pod_name, from, to);
    let form = Form::new()
        .text("podName", pod_name)
        .text("filePath", from);
    let response = dfs_client.post(
            format!("{}/v1/file/download", config.endpoint)
        )
        .header("Cookie", cookie)
        .multipart(form)
        .send()
        .await?;
    if true == response.status().is_success() {
        if let Some(content_length) = response.content_length() {
            println!("Content length: `{content_length}` bytes");
        }
        let mut stream = response.bytes_stream();
        let mut file = File::create(to)?;
        while let Some(chunk) = stream.next().await {
            file.write_all(&chunk?)?;
        }
    } else {
        return Err(format!("Download file error: `{:?}`",
            response.json::<ResponseMessage>().await?.message).as_str().into()
        );
    }

    println!("File downlaod was successful.");
    Ok(())
}

pub async fn upload_file(
    dfs_client: &reqwest::Client,
    config: &Config,
    cookie: &String,
    pod_name: String,
    dest_path: String,
    local_filename: String
) -> Result<(), Box<dyn Error>> {
    println!("Upload file request, src: `{}`, dst: `{}@{}`",
        local_filename, pod_name, dest_path);
    let file = TFile::open(local_filename.clone()).await?;
    let file_part = Part::stream(file)
        .file_name(local_filename)
        .mime_str("application/octet-stream")?;
    let form = Form::new()
        .text("podName", pod_name)
        .text("dirPath", dest_path)
        .text("blockSize", "1000000") // 1 MB
        .part("files", file_part);        
    let response = dfs_client.post(
            format!("{}/v1/file/upload", config.endpoint)
        )
        .header("Cookie", cookie)
        .multipart(form)
        .send()
        .await?;
    if false == response.status().is_success() {
        return Err(format!("Upload file error: `{:?}`",
            response.json::<ResponseMessage>().await?.message).as_str().into()
        );
    }

    println!("File upload was successful.") ;
    Ok(())
}
