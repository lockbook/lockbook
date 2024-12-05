use lb_rs::blocking::Lb;
use regex::Regex;
use serde::{Deserialize, Serialize};

pub type Graph = Vec<LinkNode>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinkNode {
    pub id: usize,
    pub title: String,
    pub links: Vec<usize>,
    pub color: [f32; 3],
    pub cluster_id: Option<usize>,
    pub internal: bool,

    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub fx: Option<f32>,
    pub fy: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct NameId {
    pub id: usize,
    pub name: String,
    pub links: Vec<usize>,
    pub internal: bool,
}

impl NameId {
    fn new(id: usize, name: String, links: Vec<usize>) -> Self {
        NameId { id, name, links, internal: true }
    }
}

impl LinkNode {
    fn new(id: usize, title: String, links_given: Vec<usize>) -> Self {
        LinkNode {
            id,
            title,
            links: links_given.clone(),
            color: [0.0, 0.0, 0.0],
            cluster_id: None,
            internal: true,

            x: 0.0, // Set to an initial value, e.g., random or based on index
            y: 0.0, // Set to an initial value
            vx: 0.0,
            vy: 0.0,
            fx: None,
            fy: None,
        }
    }
}

pub fn lockbookdata(core: &Lb) -> Graph {
    let mut graph: Graph = Vec::new();
    let mut classify: Vec<NameId> = Vec::new();
    // let core = core();
    let mut id: usize = 0;
    let mut _num_links = 1;
    let mut info: Vec<(String, String)> = Vec::new();

    for file in core.list_metadatas().unwrap() {
        if file.is_document() && file.name.ends_with(".md") {
            let doc = core.read_document(file.id).unwrap();
            let doc = String::from_utf8(doc).unwrap();
            let name = file.name;
            info.push((name, doc));
        }
    }

    info.sort_by(|a, b| a.0.cmp(&b.0));
    for n in info {
        // Check for links in the document
        let doc = n.1;
        let name = n.0;
        let links = checkforlinks(&mut classify, &mut id, &doc);
        id += 1;
        _num_links += links.len();
        classify.push(NameId::new(classify.len(), name.clone(), links));
    }
    // Add remaining links in classify to the graph if they don't exist
    for item in classify.iter() {
        let links = item.links.clone();
        if item.links.contains(&item.id) {
            let links = remove(links, &item.id);

            graph.push(LinkNode::new(item.id, item.name.to_string(), links));
        } else {
            graph.push(LinkNode::new(item.id, item.name.to_string(), links));
        }
    }

    graph
}

fn remove(links: Vec<usize>, id: &usize) -> Vec<usize> {
    let mut output = links.clone();
    let mut index;
    let mut count = 0;
    for link in links {
        if &link == id {
            index = count;
            output.remove(index);
        }
        count += 1;
    }
    return output;
}

fn checkforlinks(classify: &mut Vec<NameId>, id: &mut usize, doc: &str) -> Vec<usize> {
    let mut links: Vec<usize> = Vec::new();

    // Find all links in the document
    let link_names = find_links(doc);

    for link in link_names {
        // Check if the link is already in classify
        let link_id = in_classify(&link, &classify);

        if let Some(link_id) = link_id {
            // Ensure no duplicate links
            if !links.contains(&link_id) {
                links.push(link_id);
            }
        } else {
            links.push(*id);
            *id += 1;
            // If link not found, add it
            classify.push(NameId::new(classify.len(), link.clone(), vec![]));
        }
    }

    links
}

fn find_links(text: &str) -> Vec<String> {
    // Regex pattern to match most common types of URLs
    let url_pattern = r"(https?://|lb:)[^\s/$.?#].[^\s]*";
    let re = Regex::new(url_pattern).unwrap();

    // Collect all the matches into a Vec<String>
    let links: Vec<String> = re
        .find_iter(text)
        .map(|mat| {
            let url = mat.as_str().to_string();
            // Extract the website name from the URL
            extract_website_name(&url)
        })
        .collect();

    // Print each website name found
    // Return the website names
    links
}

fn extract_website_name(url: &str) -> String {
    // Remove "https://" or "http://" or "www." from the URL
    let domain = url
        .replace("https://", "")
        .replace("http://", "")
        .replace("www.", "");

    // Split by slashes and get the first part, which is the domain
    let parts: Vec<&str> = domain.split('/').collect();
    let domain_name = parts[0];

    // Get the base domain (youtube.com -> youtube) by splitting by dot and ignoring TLDs
    let name_parts: Vec<&str> = domain_name.split('.').collect();
    if name_parts.len() > 1 {
        name_parts[name_parts.len() - 2].to_string() // Extracts the main domain part
    } else {
        domain_name.to_string() // Fallback if no dots found
    }
}

fn in_classify(name: &String, classify: &Vec<NameId>) -> Option<usize> {
    // Search for the link in the classify vector and return its ID if found
    let mut id: Option<usize> = None;
    for linkinfo in classify {
        if &linkinfo.name == name {
            let optional_num: Option<usize> = Some(linkinfo.id);
            id = optional_num;
            break;
        }
    }
    id
}
