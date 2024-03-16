use anyhow::Result;
use url::Url;
use std::path::{Path, PathBuf};

use crate::*;

/// Abstraction for finding and downloading updates from a package source / repository.
/// An implementation may copy a file from a local repository, download from a web address,
/// or even use third party services and parse proprietary data to produce a package feed.
pub trait UpdateSource: Clone + Send + Sync {
    /// Retrieve the list of available remote releases from the package source. These releases
    /// can subsequently be downloaded with download_release_entry.
    fn get_release_feed(&self, channel: &str, app: &manifest::Manifest) -> Result<VelopackAssetFeed>;
    /// Download the specified VelopackAsset to the provided local file path.
    fn download_release_entry<A>(&self, asset: &VelopackAsset, local_file: &str, progress: A) -> Result<()>
    where
        A: FnMut(i16);
}

#[derive(Clone)]
/// Retrieves updates from a static file host or other web server.
/// Will perform a request for '{baseUri}/RELEASES' to locate the available packages,
/// and provides query parameters to specify the name of the requested package.
pub struct HttpSource {
    url: String,
}

impl HttpSource {
    /// Create a new HttpSource with the specified base URL.
    pub fn new(url: &str) -> HttpSource {
        HttpSource { url: url.to_owned() }
    }
}

impl UpdateSource for HttpSource {
    fn get_release_feed(&self, channel: &str, app: &manifest::Manifest) -> Result<VelopackAssetFeed> {
        let releases_name = format!("releases.{}.json", channel);

        let path = self.url.trim_end_matches('/').to_owned() + "/";
        let url = url::Url::parse(&path)?;
        let mut releases_url = url.join(&releases_name)?;
        releases_url.set_query(Some(format!("localVersion={}&id={}", app.version, app.id).as_str()));

        info!("Downloading releases for channel {} from: {}", channel, releases_url.to_string());
        let json = download::download_url_as_string(releases_url.as_str())?;
        let feed: VelopackAssetFeed = serde_json::from_str(&json)?;
        Ok(feed)
    }

    fn download_release_entry<A>(&self, asset: &VelopackAsset, local_file: &str, progress: A) -> Result<()>
    where
        A: FnMut(i16),
    {
        let path = self.url.trim_end_matches('/').to_owned() + "/";
        let url = url::Url::parse(&path)?;
        let asset_url = url.join(&asset.FileName)?;

        info!("About to download from URL '{}' to file '{}'", asset_url, local_file);
        download::download_url_to_file(asset_url.as_str(), local_file, progress)?;
        Ok(())
    }
}

#[derive(Clone)]
/// Retrieves available updates from a local or network-attached disk. The directory
/// must contain one or more valid packages, as well as a 'releases.{channel}.json' index file.
pub struct FileSource {
    path: PathBuf,
}

impl FileSource {
    /// Create a new FileSource with the specified base directory.
    pub fn new<P: AsRef<Path>>(path: P) -> FileSource {
        let path = path.as_ref();
        FileSource { path: PathBuf::from(path) }
    }
}

impl UpdateSource for FileSource {
    fn get_release_feed(&self, channel: &str, _: &manifest::Manifest) -> Result<VelopackAssetFeed> {
        let releases_name = format!("releases.{}.json", channel);
        let releases_path = self.path.join(&releases_name);

        info!("Reading releases from file: {}", releases_path.display());
        let json = std::fs::read_to_string(releases_path)?;
        let feed: VelopackAssetFeed = serde_json::from_str(&json)?;
        Ok(feed)
    }

    fn download_release_entry<A>(&self, asset: &VelopackAsset, local_file: &str, mut progress: A) -> Result<()>
    where
        A: FnMut(i16),
    {
        let asset_path = self.path.join(&asset.FileName);
        info!("About to copy from file '{}' to file '{}'", asset_path.display(), local_file);
        progress(50);
        std::fs::copy(asset_path, local_file)?;
        progress(100);
        Ok(())
    }
}
#[derive(Clone)]
pub struct GithubUpdateSource {
    url: String,
}

impl GithubUpdateSource {
    /// Create a new GithubUpdateSource with the specified base URL.
    pub fn new(url: &str) -> Self {
        GithubUpdateSource { 
            url: url.to_owned() 
        }
    }

    fn get_api_base_url(&self) -> Result<String> {
        let base_url: String;

        //check for valid URL
        match Url::parse(&self.url) {
            Ok(url) => {
                //handle 2 cases:
                //if github.com url
                if url.host_str() == Some("github.com") {
                    base_url = String::from("https://api.github.com/");
                } else {
                    //if not github.com url, it's probably an enterprise server
                    base_url = format!("{}://{}/api/v3/", url.scheme(), url.host_str().unwrap());
                }

                return Ok(base_url);
            }
            Err(err) => {
                panic!("Invalid URL. We should never be here.");
            }
        }
    }
}

impl UpdateSource for GithubUpdateSource {
    fn get_release_feed(&self, channel: &str, app: &manifest::Manifest) -> Result<VelopackAssetFeed> {
        let per_page = 10;
        let page = 1;
        let releases_path = format!("repos/{}/releases?per_page{per_page}&page={page}", self.url);
        let base_path = self.get_api_base_url()?;
        let get_releases_uri = format!("{base_path}{releases_path}");
        let response = download::download_url_as_string(&get_releases_uri)?;
        let releases: VelopackAssetFeed = serde_json::from_str(&response)?;
        Ok(releases)
    }

    fn download_release_entry<A>(&self, asset: &VelopackAsset, local_file: &str, progress: A) -> Result<()>
        where A: FnMut(i16)
    {
        let path = self.url.trim_end_matches('/').to_owned() + "/";
        let url = url::Url::parse(&path)?;
        let asset_url = url.join(&asset.FileName)?;

        info!("About to download from URL '{}' to file '{}'", asset_url, local_file);
        download::download_url_to_file(asset_url.as_str(), local_file, progress)?;
        Ok(())
    }
}

#[test]
fn test_get_api_base_url(){
    let normal_gh_url = "https://github.com/velopack/velopack/";
    let enterprise_gh_url = "http://internal.github.server.local/";

    let normal_gh_source = GithubUpdateSource::new(normal_gh_url);
    let enterprise_gh_source = GithubUpdateSource::new(enterprise_gh_url);

    assert_eq!("https://api.github.com/",normal_gh_source.get_api_base_url().unwrap());
    assert_eq!("http://internal.github.server.local/api/v3/",enterprise_gh_source.get_api_base_url().unwrap());
}
