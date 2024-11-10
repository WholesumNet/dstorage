use std::{
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
use futures_util::StreamExt;

use indicatif::{ProgressBar, ProgressStyle};

use anyhow;

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct Config {
    pub apiKey: String,
}


#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct ResponseMessage {
    pub message: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct FileUploadResponse {
    pub Name: String,
    pub Hash: String,
    pub Size: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadResult {
    pub id: String,
    pub name: String,
    pub cid: String,
    pub size: String,
}

pub async fn upload_file(
    client: &reqwest::Client,
    api_key: &str,
    local_filepath: String,
    dest_file_name: String
) -> anyhow::Result<UploadResult> {
    println!("Upload file request for `{}`", local_filepath);
    const UPLOAD_URL: &str = "https://node.lighthouse.storage/api/v0/add";
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
        .file_name(dest_file_name.clone())
        .mime_str("application/octet-stream")?;
    let form = Form::new()
        .part("FileData", part);  
    let resp = client.post(UPLOAD_URL)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await?;
    if false == resp.status().is_success() {
        anyhow::bail!("Upload file error: `{:?}`",
            resp.json::<ResponseMessage>().await?.message
        );
    }
    let upload_resp = resp
        .json::<FileUploadResponse>().await?;
    println!("File upload: {upload_resp:#?}");
    // dest_file_name is actually a helper to be used as job_id
    Ok(UploadResult {
        id: dest_file_name,
        name: upload_resp.Name,
        cid: upload_resp.Hash,
        size: upload_resp.Size,
    })
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
    cid: String
) -> anyhow::Result<FileInfoResponse> {
    const INFO_URL: &str = "https://api.lighthouse.storage/api/lighthouse/file_info";
    let response = client.get(INFO_URL)
        .query(&[("cid", cid)])
        .send()
        .await?;
    if false == response.status().is_success() {
        anyhow::bail!("Get file info error: `{:?}`",
            response.json::<ResponseMessage>().await?.message
        );
    }
    let file_info_resp = response
        .json::<FileInfoResponse>().await?;
    println!("File info: {file_info_resp:#?}");
    Ok(file_info_resp)
}

pub async fn download_file(
    client: &reqwest::Client,
    cid: &str,
    save_to: String,
) -> anyhow::Result<()> {
    const DOWNLOAD_BASE_URL: &str = "https://gateway.lighthouse.storage/ipfs/";
    let url = format!("{DOWNLOAD_BASE_URL}{cid}");
    let resp = client.get(url.clone())
        .send()
        .await?;
    if false == resp.status().is_success() {
        anyhow::bail!("Download file info error: `{:?}`",
            resp.json::<ResponseMessage>().await?.message
        );
    }
    let content_length = resp.content_length().unwrap_or_default();
    println!("content length: `{content_length}`");
    // setup progress bar
    let pb = ProgressBar::new(content_length);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?
        .progress_chars("#>-"));
    pb.set_message(format!("Downloading {}", url));

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
    pb.finish_with_message(format!("Downloaded `{url}` and saved it to `{save_to}`"));
    Ok(())
}