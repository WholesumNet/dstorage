use std::{
    error::Error,
    time::Duration,
    fs::File,
    io::Write,
    cmp::min,
};
use serde::Deserialize; 
use reqwest;
use reqwest::{
    multipart::{Part, Form},
};

use tokio::fs::File as TFile;
use tokio_util::io::ReaderStream;
use async_stream::stream;
use futures_util::StreamExt;

use indicatif::{ProgressBar, ProgressStyle};

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
    let file_len = file.metadata().await?.len();

    // setup progress bar
    let pb = ProgressBar::new(file_len);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?
        .progress_chars("#>-"));
    pb.set_message(format!("Uploading {}", local_filepath.clone()));

    let mut reader_stream = ReaderStream::new(file);
    let mut uploaded: u64 = 0;
    let async_stream = async_stream::stream! {
        while let Some(chunk) = reader_stream.next().await {
            if let Ok(chunk) = &chunk {
                uploaded += chunk.len() as u64;
                let progress = min(uploaded + (chunk.len() as u64), file_len);
                pb.set_position(progress);
            }
            yield chunk;
        }
        pb.finish_with_message(format!("Uploaded {}.", local_filepath.clone()));
    };

    let part = Part::stream(reqwest::Body::wrap_stream(async_stream))
        .file_name(dest_file_name)
        .mime_str("application/octet-stream")?;
    let form = Form::new()
        .text("resourceName", "filename.filetype")
        .part("FileData", part);  
    let resp = client.post(url_endpoint)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await?;
    if false == resp.status().is_success() {
        return Err(format!("Upload file error: `{:?}`",
            resp.json::<ResponseMessage>().await?.message).as_str().into()
        );
    }
    let upload_resp = resp
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
    //@ should be string but panics when mime_type is absent and requires type to be bool, why?
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
    let resp = client.get(url_endpoint.clone())
        .send()
        .await?;
    if false == resp.status().is_success() {
        return Err(format!("Download file info error: `{:?}`",
            resp.json::<ResponseMessage>().await?.message).as_str().into()
        );
    }
    let content_length = resp.content_length().unwrap_or_default();
    // setup progress bar
    let pb = ProgressBar::new(content_length);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?
        .progress_chars("#>-"));
    pb.set_message(format!("Downloading {}", url_endpoint));

    let mut stream = resp.bytes_stream();
    let mut file = File::create(save_to.clone())?;
    let mut downloaded: u64 = 0;
    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk)?;
        let progress = min(downloaded + (chunk.len() as u64), content_length);
        downloaded = progress;
        pb.set_position(progress);
    }
    pb.finish_with_message(format!("Downloaded {} to {}", url_endpoint, save_to));
    
    Ok(())
}