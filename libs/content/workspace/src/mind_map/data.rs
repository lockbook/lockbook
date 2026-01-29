use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use linkify::{LinkFinder, LinkKind};
use num_cpus;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use web_time::Duration;

lazy_static::lazy_static! {
    pub static ref URL_NAME_STORE: Mutex<Vec<LinkInfo>> = Mutex::new(Vec::new());
    pub static ref DONE: AtomicBool = AtomicBool::new(false);
    pub static ref STOP: AtomicBool = AtomicBool::new(false);
}

pub type Graph = Vec<LinkNode>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinkNode {
    pub id: usize,
    pub title: String,
    pub links: Vec<usize>,
    pub color: [f32; 3],
    pub cluster_id: Option<usize>,
    pub internal: bool,
    pub file_id: Option<Uuid>,
}

#[derive(Clone, Debug)]
pub struct NameId {
    pub id: usize,
    pub name: String,
    pub links: Vec<usize>,
    pub internal: bool,
    pub file_id: Option<Uuid>,
    pub url: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LinkInfo {
    pub id: usize,
    pub url: String,
    pub name: String,
    pub found: bool,
}

impl NameId {
    fn new(
        id: usize, name: String, links: Vec<usize>, file_id: Option<Uuid>, url: Option<String>,
    ) -> Self {
        NameId { id, name, links, internal: true, file_id, url }
    }
}

impl LinkNode {
    fn new(id: usize, title: String, links_given: Vec<usize>, file_id: Option<Uuid>) -> Self {
        LinkNode {
            id,
            title,
            links: links_given,
            color: [0.0, 0.0, 0.0],
            cluster_id: None,
            internal: true,
            file_id,
        }
    }
}

pub fn lockbook_data(core: &Lb) -> Graph {
    let mut graph: Graph = Vec::new();
    let mut classify: Vec<NameId> = Vec::new();
    let mut id: usize = 0;
    let mut info: Vec<(String, String, Uuid)> = Vec::new();

    for file in core.list_metadatas().unwrap() {
        if file.is_document() && file.name.ends_with(".md") {
            let file_id = file.id;
            let doc = core.read_document(file.id, false).unwrap();
            let doc = String::from_utf8(doc).unwrap();
            let name = file.name;
            info.push((name, doc, file_id));
        }
    }

    info.sort_by(|a, b| a.0.cmp(&b.0));
    for n in info {
        let doc = n.1;
        let name = n.0;
        let file_id = n.2;
        let links = check_for_links(&mut classify, &mut id, &doc);
        id += 1;
        classify.push(NameId::new(classify.len(), name.clone(), links, Some(file_id), None));
    }

    // Populate the global URL_NAME_STORE
    let mut store = URL_NAME_STORE.lock().unwrap();
    *store = classify
        .iter()
        .filter_map(|item| {
            item.url.as_ref().map(|url| LinkInfo {
                id: item.id,
                url: url.clone(),
                name: item.name.clone(),
                found: false,
            })
        })
        .collect();

    for item in classify.iter() {
        let links = item.links.clone();
        if item.links.contains(&item.id) {
            let links = remove(links, &item.id);
            graph.push(LinkNode::new(item.id, item.name.to_string(), links, item.file_id));
        } else {
            graph.push(LinkNode::new(item.id, item.name.to_string(), links, item.file_id));
        }
    }

    graph
}

fn remove(links: Vec<usize>, id: &usize) -> Vec<usize> {
    let mut output = links.clone();
    if let Some(pos) = output.iter().position(|x| x == id) {
        output.remove(pos);
    }
    output
}

fn check_for_links(classify: &mut Vec<NameId>, id: &mut usize, doc: &str) -> Vec<usize> {
    let mut links: Vec<usize> = Vec::new();
    let link_names = find_links(doc);

    for link in link_names {
        let url = link.clone();
        let link = extract_website_name(&link);
        let parts: Vec<&str> = link.split('/').collect();
        let domain_name: &str = if parts.len() > 1 {
            Box::leak(format!("{}/{}", parts[0], parts[1]).into_boxed_str())
        } else {
            parts[0]
        };
        let link = domain_name;
        let link_id = in_classify(&link.to_string(), classify);
        if let Some(link_id) = link_id {
            if !links.contains(&link_id) {
                links.push(link_id);
            }
        } else {
            links.push(*id);
            *id += 1;
            classify.push(NameId::new(classify.len(), link.to_string(), vec![], None, Some(url)));
        }
    }

    links
}

fn find_links(text: &str) -> Vec<String> {
    let mut finder = LinkFinder::new();
    finder.kinds(&[LinkKind::Url]);

    finder
        .links(text)
        .filter_map(|link| {
            let url = link.as_str();
            if url.starts_with("http://") || url.starts_with("https://") {
                Some(url.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn extract_website_name(url: &str) -> String {
    let domain = url
        .replace("https://", "")
        .replace("http://", "")
        .replace("www.", "");

    domain.to_string()
}

fn in_classify(name: &String, classify: &[NameId]) -> Option<usize> {
    classify.iter().find_map(
        |linkinfo| {
            if &linkinfo.name == name { Some(linkinfo.id) } else { None }
        },
    )
}

pub fn stop_extraction(val: bool) {
    STOP.store(val, Ordering::SeqCst);
}

pub fn start_extraction_names() {
    stop_extraction(false);
    let link_infos: Vec<LinkInfo> = URL_NAME_STORE.lock().unwrap().clone();
    let rt = Runtime::new().unwrap();

    // Workers take one work unit at a time from the queue and send the results back over the channel
    // Work units are represented as indexes into the link_infos array, so the queue is represented as simply the next
    // index to be processed.

    let queue = Arc::new(AtomicIsize::new((link_infos.len() - 1) as isize));
    let max_workers = std::cmp::max(1, num_cpus::get() / 2);
    let active_tasks = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    for _ in 0..link_infos.len() {
        let mut active = active_tasks.load(Ordering::SeqCst);
        while active == max_workers {
            active = active_tasks.load(Ordering::SeqCst);
        }
        if STOP.load(Ordering::SeqCst) {
            // println!("stoppped in data.rs");
            break;
        }
        let queue = queue.clone();
        active_tasks.fetch_add(1, Ordering::SeqCst);
        let async_clone_task = active_tasks.clone();
        handles.push(rt.spawn(async move {
            // Atomically grab a work unit from the queue
            let index = queue.fetch_sub(1, Ordering::SeqCst);
            if index <= 0 {
                return;
            }

            // Perform the work and put the results directly into the global store
            let index = (index - 1) as usize;
            let clone = URL_NAME_STORE.lock().unwrap().clone();
            let name = fetch_title(&clone[index].url).await;
            async_clone_task.fetch_sub(1, Ordering::SeqCst);
            match name {
                Ok(name) => {
                    // Update the global store with the result, re-locking as to not hold a lock across an await
                    let mut store = URL_NAME_STORE.lock().unwrap();
                    store[index].found = true;
                    store[index].name = name.clone();
                    // println!("{}", name.clone());
                }
                Err(_name) => {
                    // println!("{:?}", name)
                }
            }
        }));
    }

    // Block until all tasks complete
    rt.block_on(async {
        for handle in handles {
            handle.await.unwrap();
        }
    });
    DONE.store(true, Ordering::SeqCst);
    // let link_infos: Vec<LinkInfo> = URL_NAME_STORE.lock().unwrap().clone();
    // println!("{:?}\n", link_infos);
}

use reqwest;

async fn fetch_title(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Normalize the URL: if it doesn't start with "http" or "https", add "http://"
    let normalized_url = if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("https://{url}")
    };
    // println!("{}", normalized_url);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                     AppleWebKit/537.36 (KHTML, like Gecko) \
                     Chrome/115.0 Safari/537.36",
        )
        .build()?;

    let response = client
        .get(&normalized_url)
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .send()
        .await?;

    if !response.status().is_success() {
        // Return the normalized URL if the request fails
        // println!("{}", normalized_url);
        return Ok(normalized_url);
    }

    let body = response.text().await?;
    let document = Html::parse_document(&body);

    // Look for the first <title> tag
    let title_selector = Selector::parse("title").unwrap();
    if let Some(title_elem) = document.select(&title_selector).next() {
        let title = title_elem
            .text()
            .collect::<Vec<_>>()
            .join("")
            .trim()
            .to_string();
        if !title.is_empty() {
            return Ok(title);
        } else {
            // println!("{}", normalized_url);
        }
    }

    // If no title found, return the normalized URL
    Ok(normalized_url)
}
