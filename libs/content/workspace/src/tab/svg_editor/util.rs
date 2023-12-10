use minidom::Element;

pub fn node_by_id(root: &mut Element, id: String) -> Option<&mut Element> {
    root.children_mut().find(
        |e| {
            if let Some(id_attr) = e.attr("id") {
                id_attr == id
            } else {
                false
            }
        },
    )
}
