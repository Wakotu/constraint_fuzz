use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::{
    analysis::constraint::inter::exec_tree::{
        action::ExecAction, incre_dot_counter, DotId, ExecTree, FuncIter, FuncNode,
        SharedFuncNodePtr, SubFuncIter,
    },
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
        // self.len.partial_cmp(&other.len)
        other.len.partial_cmp(&self.len) // reverse order for descending sort
    }
}

impl Ord for FuncNodeLenEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // self.len.cmp(&other.len)
        other.len.cmp(&self.len) // reverse order for descending sort
    }
}

pub struct FuncNodeLenList {
    data: Vec<FuncNodeLenEntry>,
}

impl std::fmt::Display for FuncNodeLenList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "FuncNodeLenList: {} entries", self.data.len(),)?;
        for entry in self.data.iter() {
            writeln!(
                f,
                "Function: {}, Length: {}",
                entry.get_func_name(),
                entry.get_len()
            )?;
        }
        Ok(())
    }
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
    // const INTRA_ACTION_LIMIT: usize = 50;
    // const LOOP_ACTION_LIMIT: usize = 30;

    fn should_ignore_act(cur_func: &FuncNode, idx: usize) -> Result<bool> {
        // ignore action if it is a loop or intra-function action
        let act = cur_func.get_act_at(idx).ok_or_else(|| {
            eyre::eyre!(
                "Action at index {} does not exist in function: {}",
                idx,
                cur_func.get_dot_id()
            )
        })?;
        match act {
            // ignore intra-function actions
            ExecAction::Func(_) => Ok(false),
            // ignore loop actions
            ExecAction::Loop(_) => Ok(true),
            ExecAction::Intra(_) => Ok(true),
        }
    }

    fn get_ellipsis_dot_id(ignore_cnt: usize) -> String {
        let suffix = incre_dot_counter();
        format!("\"{}......{}\"", suffix, ignore_cnt)
    }

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

        // draw action nodes along with edges inside current Function cluster
        {
            let mut cluster = digraph.cluster();
            cluster.node_attributes().set_style(Style::Filled);

            // draw function -> action edges
            let mut ignore_prev = 0;

            for (idx, act_id) in act_id_list.iter().enumerate() {
                // judge if current action node should be ignored
                if Self::should_ignore_act(&cur_func, idx)? {
                    ignore_prev += 1;
                    continue;
                }
                if ignore_prev > 0 {
                    cluster.edge(&cur_func_id, Self::get_ellipsis_dot_id(ignore_prev));
                    ignore_prev = 0;
                }
                cluster.edge(&cur_func_id, act_id);
            }
            // label length of current function
            cluster.set_label(&format!(
                "Function: {}, Length: {}",
                cur_func_id,
                cur_func.get_len()
            ));
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
        if !output.stdout.is_empty() {
            log::warn!(
                "Dot command output: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
        if !output.stderr.is_empty() {
            log::warn!(
                "Dot command error output: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        if !output.status.success() {
            bail!(
                "Failed to convert dot file to svg: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }

    /// Iterated element: FuncNode
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

    pub fn show_long_func_nodes(&self) -> Result<()> {
        let func_len_list = self.collect_long_func_nodes()?;
        log::info!("{}", func_len_list);
        Ok(())
    }

    pub fn collect_recur_entries(&self) -> Result<RecurRes> {
        let recur_checker = ExecTreeRecurChecker::new();
        recur_checker.check_recur(self.root_ptr.clone())
    }

    pub fn show_recur_entries(&self) -> Result<()> {
        let recur_entries = self.collect_recur_entries()?;
        if recur_entries.is_empty() {
            log::info!("No recursion entries found in the execution tree.");
        } else {
            log::info!("{} Recursion entries found:", recur_entries.len());
            for entry in recur_entries.iter() {
                log::info!(
                    "Function Cycle: {:?}, Parent Function: {}",
                    entry.func_cycle,
                    entry.parent_func
                );
            }
        }
        Ok(())
    }

    pub fn show_most_called_funcs(&self) -> Result<()> {
        const FUNC_NUM: usize = 10;

        let mut func_call_count: HashMap<String, usize> = HashMap::new();

        for func_node_ptr in self.bfs_iter() {
            let func_node = func_node_ptr.borrow();
            let func_name = func_node.get_func_name_or_init();
            let count = func_call_count.entry(func_name.to_owned()).or_insert(0);
            *count += 1;
        }

        // sort by count
        let mut sorted_funcs: Vec<_> = func_call_count
            .into_iter()
            .map(|(name, count)| (name, count))
            .collect();
        sorted_funcs.sort_by(|a, b| b.1.cmp(&a.1)); // sort by count in descending order

        for entry in sorted_funcs.iter().take(FUNC_NUM) {
            log::info!("Function: {}, Call Count: {}", entry.0, entry.1);
        }

        let most_called_entry = sorted_funcs.first().ok_or_else(|| {
            eyre::eyre!("No functions found in the execution tree to analyze call counts.")
        })?;

        const ITER_TRY: usize = 5;

        let mut triage: usize = 0;
        for func_node_ptr in self.bfs_iter() {
            let func_node = func_node_ptr.borrow();
            if func_node.get_func_name_or_init() == most_called_entry.0 {
                triage += 1;
                if triage > ITER_TRY {
                    break;
                }
                let parent_func_ptr = func_node.get_parent_ptr().ok_or_else(|| {
                    eyre::eyre!(
                        "Function {} has no parent function",
                        func_node.get_func_name_or_init()
                    )
                })?;
                let parent_func = parent_func_ptr.borrow();
                let parent_func_name = parent_func.get_func_name_or_init();
                log::info!(
                    "Function {}, {} found, Parent Function: {}",
                    most_called_entry.0,
                    triage,
                    parent_func_name
                );
            }
        }

        Ok(())
    }
}

pub struct RecurEntry {
    func_cycle: Vec<String>,
    parent_func: String,
}

type RecurRes = Vec<RecurEntry>;

pub struct ExecTreeRecurChecker {
    recur_entries: Vec<RecurEntry>,
    // root_ptr: SharedFuncNodePtr,
    // stack of function names to track recursion
    func_stack: Vec<String>,
}

impl ExecTreeRecurChecker {
    fn new() -> Self {
        Self {
            recur_entries: Vec::new(),
            func_stack: Vec::new(),
        }
    }

    /// update function stack during traversion and add recur entries detected to global list
    fn check_recur_impl(&mut self, cur_func_ptr: SharedFuncNodePtr) -> Result<()> {
        let cur_func = cur_func_ptr.borrow();

        let func_name = cur_func.get_func_name_or_init().to_owned();
        // check recursion
        if self.func_stack.contains(&func_name) {
            // recursion detected
            let mut func_cycle = Vec::new();
            // find the cycle start index
            let cycle_start_idx = self
                .func_stack
                .iter()
                .position(|x| x == &func_name)
                .ok_or_else(|| {
                    eyre::eyre!(
                        "Function {} contained not found in function stack: {:?}",
                        func_name,
                        self.func_stack
                    )
                })?;
            // collect the cycle entries
            for entry in self.func_stack.iter().skip(cycle_start_idx) {
                func_cycle.push(entry.to_owned());
            }
            // add parent function name
            let parent_func = self
                .func_stack
                .get(cycle_start_idx - 1)
                .map(|s| s.to_owned())
                .ok_or_else(|| {
                    eyre::eyre!(
                        "Parent function not found for recursion in stack: {:?}",
                        self.func_stack
                    )
                })?;
            // create a new recur entry
            let recur_entry = RecurEntry {
                func_cycle,
                parent_func,
            };
            self.recur_entries.push(recur_entry);
        }
        // push to stack
        self.func_stack.push(func_name);

        for sub_func_ptr in cur_func_ptr.iter_sub_funcs() {
            self.check_recur_impl(sub_func_ptr.clone())?;
        }

        // stack pop
        self.func_stack.pop();
        Ok(())
    }

    pub fn check_recur(mut self, root_func_ptr: SharedFuncNodePtr) -> Result<RecurRes> {
        self.check_recur_impl(root_func_ptr)?;
        Ok(self.recur_entries)
    }
}
