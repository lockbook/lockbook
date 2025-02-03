use lb_rs::{blocking::Lb, Uuid};
use linkify::{LinkFinder, LinkKind};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicIsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::runtime::Runtime;

lazy_static::lazy_static! {
    pub static ref URL_NAME_STORE: Mutex<Vec<LinkInfo>> = Mutex::new(Vec::new());
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

#[derive(Clone)]
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
                Some(extract_website_name(url))
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

fn in_classify(name: &String, classify: &Vec<NameId>) -> Option<usize> {
    classify.iter().find_map(
        |linkinfo| {
            if &linkinfo.name == name {
                Some(linkinfo.id)
            } else {
                None
            }
        },
    )
}

pub fn start_extraction_names() {
    let link_infos: Vec<LinkInfo> = URL_NAME_STORE.lock().unwrap().clone();
    let rt = Runtime::new().unwrap();

    // Workers take one work unit at a time from the queue and send the results back over the channel
    // Work units are represented as indexes into the link_infos array, so the queue is represented as simply the next
    // index to be processed.
    let queue = Arc::new(AtomicIsize::new((link_infos.len() - 1) as isize));
    let mut handles = vec![];

    const NUM_WORKERS: usize = 100;
    for _ in 0..NUM_WORKERS {
        let queue = queue.clone();
        handles.push(rt.spawn(async move {
            // Atomically grab a work unit from the queue
            let index = queue.fetch_sub(1, Ordering::SeqCst);
            if index <= 0 {
                return;
            }

            // Perform the work and put the results directly into the global store
            let index = (index - 1) as usize;
            let clone = URL_NAME_STORE.lock().unwrap().clone();
            if let Ok(name) = get_url_names(&clone[index].url).await {
                // Update the global store with the result, re-locking as to not hold a lock across an await
                let mut store = URL_NAME_STORE.lock().unwrap();
                store[index].found = true;
                store[index].name = name;
            }
        }));
    }

    // Block until all tasks complete
    rt.block_on(async {
        for handle in handles {
            handle.await.unwrap();
        }
    });
}

async fn get_url_names(
    url: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let client = Client::builder()
        .timeout(Duration::from_millis(250))
        .build()?;

    let response = client.get(url).send().await?;
    let body = response.bytes().await?;
    let title_re = Regex::new(r"<title>(.*?)</title>")?;

    let title = match title_re.captures(&String::from_utf8_lossy(&body)) {
        Some(caps) => caps[1].trim().to_string(),
        None => url.to_owned(),
    };

    Ok(title)
}
