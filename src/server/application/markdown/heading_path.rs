use super::{Block, HeadingBlock};

pub fn update_heading_path(path: &mut Vec<String>, heading: &HeadingBlock) {
    let depth = heading.depth.max(1) as usize;
    path.truncate(depth - 1);
    while path.len() < depth {
        path.push(String::new());
    }
    if let Some(slot) = path.get_mut(depth - 1) {
        slot.clone_from(&heading.text);
    }
}

pub fn join_heading_path(path: &[String]) -> String {
    path.iter()
        .filter(|item| !item.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" > ")
}

pub fn collect_block_text(current: &[&Block]) -> String {
    current.iter().map(|block| block.text.as_str()).collect()
}
