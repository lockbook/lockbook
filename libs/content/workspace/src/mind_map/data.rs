use lb_rs::{blocking::Lb, Uuid};
use linkify::{LinkFinder, LinkKind};
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
    pub file_id: Option<Uuid>,
}

#[derive(Clone, Debug)]
pub struct NameId {
    pub id: usize,
    pub name: String,
    pub links: Vec<usize>,
    pub internal: bool,
    pub file_id: Option<Uuid>,
}

impl NameId {
    fn new(id: usize, name: String, links: Vec<usize>, file_id: Option<Uuid>) -> Self {
        NameId { id, name, links, internal: true, file_id }
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
            let doc = core.read_document(file.id).unwrap();
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
        classify.push(NameId::new(classify.len(), name.clone(), links, Some(file_id)));
    }
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
    let mut index;
    for (count, link) in links.into_iter().enumerate() {
        if &link == id {
            index = count;
            output.remove(index);
        }
    }
    output
}

fn check_for_links(classify: &mut Vec<NameId>, id: &mut usize, doc: &str) -> Vec<usize> {
    let mut links: Vec<usize> = Vec::new();
    let link_names = find_links(doc);

    for link in link_names {
        let link_id = in_classify(&link, classify);
        if let Some(link_id) = link_id {
            if !links.contains(&link_id) {
                links.push(link_id);
            }
        } else {
            links.push(*id);
            *id += 1;
            classify.push(NameId::new(classify.len(), link.clone(), vec![], None));
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

    let parts: Vec<&str> = domain.split('/').collect();
    let domain_name: &str = if parts.len() > 1 {
        Box::leak(format!("{}/{}", parts[0], parts[1]).into_boxed_str())
    } else {
        parts[0]
    };
    domain_name.to_string()
}

fn in_classify(name: &String, classify: &Vec<NameId>) -> Option<usize> {
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
