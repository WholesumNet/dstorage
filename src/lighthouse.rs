use std::{
    error::Error,
    time::Duration,
    fs::File,
    io::Write,
};
use serde::Deserialize; 
use reqwest;
use reqwest::{
    multipart::{Part, Form},
};

use tokio::fs::File as TFile;
use futures_util::StreamExt;

use serde_json::json;

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct ResponseMessage {
    pub message: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct FileUploadResponse {
    pub Name: String,
    pub Hash: String,
    pub Size: String,
}

pub async fn upload_file(
    client: &reqwest::Client,
    url_endpoint: String,
    api_key: String,
    local_filepath: String,
    dest_file_name: String
) -> Result<FileUploadResponse, Box<dyn Error>> {
    println!("Upload file request for `{}`", local_filepath);
    let file = TFile::open(local_filepath.clone()).await?;
    // let mut headers = HeaderMap::new();
    // headers.insert("MimeType", mime_type.parse().unwrap());
    let file_part = Part::stream(file)
        .file_name(dest_file_name)
        .mime_str("application/octet-stream")?;
    let form = Form::new()
        .part("files", file_part);        
    let response = client.post(url_endpoint)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await?;
    if false == response.status().is_success() {
        return Err(format!("Upload file error: `{:?}`",
            response.json::<ResponseMessage>().await?.message).as_str().into()
        );
    }
    let upload_resp = response
        .json::<FileUploadResponse>().await?;
    println!("File upload: {upload_resp:#?}");
    Ok(upload_resp)
}


#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct FileInfoResponse {
    pub fileSizeInBytes: String,
    pub cid: String,
    pub encryption: bool,
    pub fileName: String,
    //@ should be string but panics and bool works, why?
    pub mimeType: bool,
}

pub async fn get_file_info(
    client: &reqwest::Client,
    url_endpoint: String,
    cid: String
) -> Result<FileInfoResponse, Box<dyn Error>> {
    let response = client.get(url_endpoint)
        .query(&[("cid", cid)])
        .send()
        .await?;
    if false == response.status().is_success() {
        return Err(format!("Get file info error: `{:?}`",
            response.json::<ResponseMessage>().await?.message).as_str().into()
        );
    }
    // let content = response
    //     .text_with_charset("utf-8")
    //     .await?;
    // println!("{content}");
    let file_info_resp = response
        .json::<FileInfoResponse>().await?;
    println!("File info: {file_info_resp:#?}");
    Ok(file_info_resp)
}

pub async fn download_file(
    client: &reqwest::Client,
    url_endpoint: String,
    save_to: String,
) -> Result<(), Box<dyn Error>> {
    let response = client.get(url_endpoint)
        .send()
        .await?;
    if false == response.status().is_success() {
        return Err(format!("Download file info error: `{:?}`",
            response.json::<ResponseMessage>().await?.message).as_str().into()
        );
    }
    if let Some(content_length) = response.content_length() {
            println!("Content length: `{content_length}` bytes");
    }
    let mut stream = response.bytes_stream();
    let mut file = File::create(save_to)?;
    while let Some(chunk) = stream.next().await {
        file.write_all(&chunk?)?;
    }
    
    Ok(())
}