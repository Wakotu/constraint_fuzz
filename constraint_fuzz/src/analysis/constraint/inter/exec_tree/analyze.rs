use std::path::{Path, PathBuf};

use crate::{
    analysis::constraint::inter::exec_tree::{DotId, ExecTree, SharedFuncNodePtr},
    deopt::utils::write_bytes_to_file,
};
use color_eyre::eyre::{bail, Result};
use dot_writer::{Attributes, DotWriter, Style};

/**
 * BFS Iterator
 */

pub struct ExecTreeIter {
    queue: Vec<SharedFuncNodePtr>,
}

impl ExecTreeIter {
    pub fn new(root: SharedFuncNodePtr) -> Self {
        let mut queue = Vec::new();
        queue.push(root);
        Self { queue }
    }
}

impl Iterator for ExecTreeIter {
    type Item = SharedFuncNodePtr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.queue.is_empty() {
            return None;
        }
        let cur_node_ptr = self.queue.remove(0);
        let cur_node = cur_node_ptr.borrow();
        // add children to the queue
        for act in cur_node.iter_acts() {
            if let Some(func_act) = act.get_func_call_act() {
                let child_ptr = func_act
                    .get_child_ptr()
                    .expect("Function action is a call but has no child pointer");
                self.queue.push(child_ptr);
            }
        }
        Some(cur_node_ptr.clone())
    }
}
/**
 * Long Function Node Length List
 */

#[derive(PartialEq, Eq)]
pub struct FuncNodeLenEntry {
    func_name: String,
    len: usize,
}

impl FuncNodeLenEntry {
    pub fn new(func_name: String, len: usize) -> Self {
        Self { func_name, len }
    }

    pub fn get_func_name(&self) -> &str {
        &self.func_name
    }

    pub fn get_len(&self) -> usize {
        self.len
    }
}

impl PartialOrd for FuncNodeLenEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.len.partial_cmp(&other.len)
    }
}

impl Ord for FuncNodeLenEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.len.cmp(&other.len)
    }
}

pub struct FuncNodeLenList {
    data: Vec<FuncNodeLenEntry>,
}

impl FuncNodeLenList {
    const LEN_LIMIT: usize = 10;

    pub fn new() -> Self {
        Self {
            data: Vec::with_capacity(Self::LEN_LIMIT),
        }
    }

    pub fn push(&mut self, len_entry: FuncNodeLenEntry) -> Result<()> {
        let mut flag = false;
        if self.data.len() < Self::LEN_LIMIT {
            self.data.push(len_entry);
            flag = true;
        } else {
            let least_func_ent = self.data.last().ok_or_else(|| {
                eyre::eyre!("Cannot push to FuncNodeLenList, no least entry found")
            })?;

            if len_entry.len > least_func_ent.len {
                // substitute least entry with new one
                self.data.pop();
                self.data.push(len_entry);
                flag = true;
            }
        }
        // sort the data if it was modified
        if flag {
            self.data.sort();
        }

        Ok(())
    }
}

impl ExecTree {
    fn draw_func_cluster<'d, 'w>(
        cur_func_ptr: SharedFuncNodePtr,
        digraph: &mut dot_writer::Scope<'d, 'w>,
    ) -> Result<String> {
        let cur_func = cur_func_ptr.borrow();
        let cur_func_id = cur_func.get_dot_id();
        let act_id_list = cur_func
            .iter_acts()
            .map(|act| act.get_dot_id())
            .collect::<Vec<_>>();

        {
            let mut cluster = digraph.cluster();
            cluster.node_attributes().set_style(Style::Filled);

            // draw function -> action edges
            for act_id in act_id_list.iter() {
                cluster.edge(&cur_func_id, act_id);
            }
            cluster.set_label(&cur_func_id);
        }
        // cluster.set_label(func_id);

        for (act, act_id) in cur_func.iter_acts().zip(act_id_list.iter()) {
            if let Some(func_act) = act.get_func_call_act() {
                let sub_func_ptr = func_act.get_child_ptr().ok_or_else(|| {
                    eyre::eyre!(
                        "Function action is a call but has no child pointer: {}",
                        func_act.get_name()
                    )
                })?;
                let sub_func_id = Self::draw_func_cluster(sub_func_ptr, digraph)?;
                digraph.edge(act_id, sub_func_id);
            }
        }
        Ok(cur_func_id.to_owned())
    }

    fn draw_graph<'d, 'w>(&self, digraph: &mut dot_writer::Scope<'d, 'w>) -> Result<()> {
        Self::draw_func_cluster(self.root_ptr.clone(), digraph)?;
        Ok(())
    }

    pub fn to_dot_file<P: AsRef<Path>>(&self, dot_path: P) -> Result<()> {
        log::info!("Starting to convert ExecTree to DOT format");
        let mut dot_bytes = vec![];

        // brackets to ensure that `dot_writer` is dropped before we write to the file
        {
            let mut dot_writer = DotWriter::from(&mut dot_bytes);
            dot_writer.set_pretty_print(true);
            let mut digraph = dot_writer.digraph();
            self.draw_graph(&mut digraph)?;
        }
        // write  dot_bytes to png_path
        write_bytes_to_file(dot_path.as_ref(), &dot_bytes)?;
        log::info!(
            "dot conversion completed, written to: {}",
            dot_path.as_ref().display()
        );
        Ok(())
    }

    fn get_dot_path_from_svg_path<P: AsRef<Path>>(svg_path: P) -> Result<PathBuf> {
        let svg_path = svg_path.as_ref();
        let mut dot_path = svg_path.to_owned();
        // replace .png with .dot
        if let Some(ext) = dot_path.extension() {
            if ext == "svg" {
                dot_path.set_extension("dot");
            } else {
                bail!(
                    "SVG file does not have .svg extension: {}",
                    svg_path.display()
                );
            }
        } else {
            bail!(
                "SVG file does not have an extension: {}",
                svg_path.display()
            );
        }
        Ok(dot_path)
    }

    pub fn to_dot_svg<P: AsRef<Path>>(&self, svg_path: P) -> Result<()> {
        let dot_path = Self::get_dot_path_from_svg_path(&svg_path)?;
        self.to_dot_file(&dot_path)?;

        log::debug!("Converting dot file to svg: {}", dot_path.display());
        // run dot command to convert dot file to png
        let output = std::process::Command::new("dot")
            .arg("-Tsvg")
            .arg(&dot_path)
            .arg("-o")
            .arg(svg_path.as_ref())
            .output()?;
        log::debug!(
            "Dot command output: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        log::debug!(
            "Dot command error output: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if !output.status.success() {
            bail!(
                "Failed to convert dot file to png: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }

    pub fn bfs_iter(&self) -> ExecTreeIter {
        ExecTreeIter::new(self.root_ptr.clone())
    }

    pub fn collect_long_func_nodes(&self) -> Result<FuncNodeLenList> {
        let mut func_len_list = FuncNodeLenList::new();
        for node_ptr in self.bfs_iter() {
            let node = node_ptr.borrow();
            let len_entry = node.to_len_entry();
            func_len_list.push(len_entry)?;
        }
        Ok(func_len_list)
    }
}
