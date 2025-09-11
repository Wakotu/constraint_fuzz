use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    rc::Rc,
};

use crate::{
    analysis::constraint::inter::{
        exec_tree::action::ExecAction,
        exec_tree::thread_tree::{
            incre_dot_counter, DotId, ExecFuncNode, FuncIter, SharedFuncNodePtr, ThreadExecTree,
        },
        loc::SrcLoc,
    },
    deopt::utils::write_bytes_to_file,
};
use color_eyre::eyre::{bail, Result};
use dot_writer::{Attributes, DotWriter, Style};

/**
 * BFS Iterator
 */

pub struct ThreadTreeIter {
    queue: Vec<SharedFuncNodePtr>,
}

impl ThreadTreeIter {
    pub fn new(root: SharedFuncNodePtr) -> Self {
        let mut queue = Vec::new();
        queue.push(root);
        Self { queue }
    }
}

impl Iterator for ThreadTreeIter {
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

impl ThreadExecTree {
    // const INTRA_ACTION_LIMIT: usize = 50;
    // const LOOP_ACTION_LIMIT: usize = 30;

    fn act_ignore_at_visualization(cur_func: &ExecFuncNode, idx: usize) -> Result<bool> {
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
            _ => Ok(true),
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
                if Self::act_ignore_at_visualization(&cur_func, idx)? {
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

    fn draw_func_cluster_for_func_tree<'d, 'w>(
        cur_func_ptr: SharedFuncNodePtr,
        digraph: &mut dot_writer::Scope<'d, 'w>,
    ) -> Result<String> {
        let cur_func = cur_func_ptr.borrow();
        let cur_func_id = cur_func.get_dot_id();

        // iterate all subfunctions
        for sub_func_ptr in cur_func_ptr.iter_sub_funcs() {
            // recursively draw subfunctions
            let sub_func_id = Self::draw_func_cluster_for_func_tree(sub_func_ptr, digraph)?;
            digraph.edge(&cur_func_id, sub_func_id);
        }

        Ok(cur_func_id.to_owned())
    }

    fn draw_func_tree_graph<'d, 'w>(&self, digraph: &mut dot_writer::Scope<'d, 'w>) -> Result<()> {
        Self::draw_func_cluster_for_func_tree(self.root_ptr.clone(), digraph)?;
        Ok(())
    }
    pub fn to_func_tree_dot_file<P: AsRef<Path>>(&self, dot_path: P) -> Result<()> {
        log::info!("Starting to convert ExecTree to Function-Tree DOT file");
        let mut dot_bytes = vec![];

        // brackets to ensure that `dot_writer` is dropped before we write to the file
        {
            let mut dot_writer = DotWriter::from(&mut dot_bytes);
            dot_writer.set_pretty_print(true);
            let mut digraph = dot_writer.digraph();
            self.draw_func_tree_graph(&mut digraph)?;
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
        self.to_func_tree_dot_file(&dot_path)?;

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
    pub fn func_node_bfs_iter(&self) -> ThreadTreeIter {
        ThreadTreeIter::new(self.root_ptr.clone())
    }

    pub fn collect_long_func_nodes(&self) -> Result<FuncNodeLenList> {
        let mut func_len_list = FuncNodeLenList::new();
        for node_ptr in self.func_node_bfs_iter() {
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
        log::debug!("Original root length: {}", self.root_ptr.borrow().get_len());
        let recur_checker = ThreadTreeRecurChecker::new();
        recur_checker.check_recur(self.root_ptr.clone())
    }

    pub fn show_recur_entries(&self) -> Result<()> {
        log::info!("Checking for recursions in the execution tree...");
        let recur_entries = self.collect_recur_entries()?;
        if recur_entries.is_empty() {
            log::debug!("No recursion entries found in the execution tree.");
        } else {
            log::debug!("{} Recursion entries found:", recur_entries.len());
            for entry in recur_entries.iter() {
                log::debug!(
                    "Function Cycle: {:?}, Parent Function: {}",
                    entry.func_cycle,
                    entry.parent_func
                );
            }
        }
        Ok(())
    }

    pub fn show_most_called_funcs(&self) -> Result<()> {
        log::info!("Analyzing most called functions in the execution tree...");
        const FUNC_NUM: usize = 10;

        let mut func_call_count: HashMap<String, usize> = HashMap::new();

        for func_node_ptr in self.func_node_bfs_iter() {
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
            log::debug!("Function: {}, Call Count: {}", entry.0, entry.1);
        }

        let most_called_entry = sorted_funcs.first().ok_or_else(|| {
            eyre::eyre!("No functions found in the execution tree to analyze call counts.")
        })?;

        // Invocation information for the most called function
        const ITER_TRY: usize = 5;

        let mut triage: usize = 0;
        for func_node_ptr in self.func_node_bfs_iter() {
            let func_node = func_node_ptr.borrow();
            if func_node.get_func_name_or_init() == most_called_entry.0 {
                triage += 1;
                if triage > ITER_TRY {
                    break;
                }
                let parent_func_ptr = match func_node.get_parent_ptr() {
                    Some(ptr) => ptr,
                    None => {
                        log::warn!(
                            "Most Called Function {} has no parent function",
                            most_called_entry.0
                        );
                        return Ok(()); // continue if no parent function found
                    }
                };
                let parent_func = parent_func_ptr.borrow();
                let parent_func_name = parent_func.get_func_name_or_init();
                log::debug!(
                    "Function {} with length of {}, {} found, Parent Function: {}",
                    most_called_entry.0,
                    func_node.get_len(),
                    triage,
                    parent_func_name
                );
            }
        }
        match self.show_common_parent_for_mcf(&most_called_entry.0) {
            Ok(_) => {}
            Err(e) => {
                log::warn!(
                    "Failed to find common parent for most called function {}: {}",
                    most_called_entry.0,
                    e
                );
            }
        }
        // self.show_common_parent_for_mcf("av1_read_coeffs_txb")?;

        Ok(())
    }

    pub fn show_most_hit_loop_headers(&self) -> Result<()> {
        log::info!("Analyzing most hit loop headers in the execution tree...");
        const LOOP_NUM: usize = 10;

        let mut loop_header_count: HashMap<SrcLoc, usize> = HashMap::new();

        for func_node_ptr in self.func_node_bfs_iter() {
            let func_node = func_node_ptr.borrow();
            for act in func_node.iter_acts() {
                if let ExecAction::Loop(loop_act) = act {
                    let header_name = loop_act.get_header_loc().to_owned();
                    let count = loop_header_count.entry(header_name).or_insert(0);
                    *count += 1;
                }
            }
        }

        // sort by count
        let mut sorted_loops: Vec<_> = loop_header_count
            .into_iter()
            .map(|(name, count)| (name, count))
            .collect();
        sorted_loops.sort_by(|a, b| b.1.cmp(&a.1)); // sort by count in descending order

        for entry in sorted_loops.iter().take(LOOP_NUM) {
            log::debug!("Loop Header: {:?}, Hit Count: {}", entry.0, entry.1);
        }

        Ok(())
    }

    pub fn show_func_with_most_childs(&self) -> Result<()> {
        log::info!("Analyzing functions with the most child functions in the execution tree...");
        const FUNC_NUM: usize = 10;

        let mut func_child_count: HashMap<String, usize> = HashMap::new();

        for func_node_ptr in self.func_node_bfs_iter() {
            let func_node = func_node_ptr.borrow();
            let func_name = func_node.get_func_name_or_init();
            let sub_count = func_node_ptr.iter_sub_funcs().count();
            func_child_count
                .entry(func_name.to_owned())
                .or_insert(sub_count);
        }

        // sort by count
        let mut sorted_funcs: Vec<_> = func_child_count
            .into_iter()
            .map(|(name, count)| (name, count))
            .collect();
        sorted_funcs.sort_by(|a, b| b.1.cmp(&a.1)); // sort by count in descending order

        for entry in sorted_funcs.iter().take(FUNC_NUM) {
            log::debug!("Function: {}, Child Count: {}", entry.0, entry.1);
        }

        Ok(())
    }

    fn show_child_to_parent(
        child: SharedFuncNodePtr,
        parent: SharedFuncNodePtr,
        prompt: &str,
    ) -> Result<()> {
        log::debug!(
            "{}: {} -> {}",
            prompt,
            child.borrow().get_func_name_or_init(),
            parent.borrow().get_func_name_or_init()
        );
        Ok(())
    }

    fn get_common_parent_ptr(
        &self,
        func_ptr_a: SharedFuncNodePtr,
        func_ptr_b: SharedFuncNodePtr,
    ) -> Result<SharedFuncNodePtr> {
        log::info!(
            "Show common parent for functions: {} and {}",
            func_ptr_a.borrow().get_func_name_or_init(),
            func_ptr_b.borrow().get_func_name_or_init()
        );
        let mut parent_ptr_a_op = func_ptr_a.borrow().get_parent_ptr();
        let mut parent_ptr_b_op = func_ptr_b.borrow().get_parent_ptr();

        let mut child_ptr_a = func_ptr_a.clone();
        let mut child_ptr_b = func_ptr_b.clone();

        while let (Some(ptr_a), Some(ptr_b)) = (parent_ptr_a_op, parent_ptr_b_op) {
            Self::show_child_to_parent(child_ptr_a.clone(), ptr_a.clone(), "Call Chain A")?;
            Self::show_child_to_parent(child_ptr_b.clone(), ptr_b.clone(), "Call Chain B")?;
            if Rc::ptr_eq(&ptr_a, &ptr_b) {
                return Ok(ptr_a);
            }

            child_ptr_a = ptr_a.clone();
            child_ptr_b = ptr_b.clone();
            parent_ptr_a_op = ptr_a.borrow().get_parent_ptr();
            parent_ptr_b_op = ptr_b.borrow().get_parent_ptr();
        }

        bail!("No common parent found for the given function pointers");
    }

    fn show_chain_to_root(func_node_ptr: SharedFuncNodePtr) -> () {
        let mut cur_ptr = func_node_ptr;
        loop {
            let parent_ptr_op = { cur_ptr.borrow().get_parent_ptr() };
            if let Some(parent_ptr) = parent_ptr_op {
                log::debug!(
                    "{} -> {}",
                    cur_ptr.borrow().get_func_name_or_init(),
                    parent_ptr.borrow().get_func_name_or_init()
                );
                cur_ptr = parent_ptr;
            } else {
                log::debug!(
                    "Reached root function: {}",
                    cur_ptr.borrow().get_func_name_or_init()
                );
                break;
            }
        }
    }

    fn show_common_parent_for_mcf(&self, func_name: &str) -> Result<()> {
        // collect 2 function pointers with the same name
        let mut func_ptrs: Vec<SharedFuncNodePtr> = Vec::new();
        for func_node_ptr in self.func_node_bfs_iter() {
            let func_node = func_node_ptr.borrow();
            if func_node.get_func_name_or_init() == func_name {
                func_ptrs.push(func_node_ptr.clone());
                if func_ptrs.len() >= 2 {
                    break; // we only need two function pointers
                }
            }
        }

        let func_ptr_a = func_ptrs
            .get(0)
            .ok_or_else(|| eyre::eyre!("Function {} not found in the execution tree", func_name))?;
        let func_ptr_b = func_ptrs
            .get(1)
            .ok_or_else(|| eyre::eyre!("Function {} not found in the execution tree", func_name))?;

        let common_parent_ptr =
            match self.get_common_parent_ptr(func_ptr_a.clone(), func_ptr_b.clone()) {
                Ok(common_parent_ptr) => {
                    log::info!(
                        "Common parent for functions {} and {}: {}",
                        func_ptr_a.borrow().get_func_name_or_init(),
                        func_ptr_b.borrow().get_func_name_or_init(),
                        common_parent_ptr.borrow().get_func_name_or_init()
                    );
                    common_parent_ptr.clone()
                }
                Err(e) => {
                    log::error!("Error finding common parent: {}", e);
                    return Err(e);
                }
            };

        Self::show_chain_to_root(common_parent_ptr.clone());
        Ok(())
    }
}

#[derive(PartialEq, Eq, Hash)]
pub struct RecurEntry {
    func_cycle: Vec<String>,
    parent_func: String,
}

type RecurRes = Vec<RecurEntry>;

pub struct ThreadTreeRecurChecker {
    recur_entries: HashSet<RecurEntry>,
    // root_ptr: SharedFuncNodePtr,
    // stack of function names to track recursion
    func_stack: Vec<String>,
}

impl ThreadTreeRecurChecker {
    fn new() -> Self {
        Self {
            recur_entries: HashSet::new(),
            func_stack: Vec::new(),
        }
    }

    fn get_latest_same_func(&self, func_name: &str) -> Option<usize> {
        // find the latest index of the same function name in the stack
        self.func_stack.iter().rposition(|x| x == func_name)
    }

    /// update function stack during traversion and add recur entries detected to global list
    fn check_recur_impl(&mut self, cur_func_ptr: SharedFuncNodePtr) -> Result<()> {
        let cur_func = cur_func_ptr.borrow();

        let func_name = cur_func.get_func_name_or_init().to_owned();
        // check recursion

        if let Some(cycle_start_idx) = self.get_latest_same_func(&func_name) {
            // recursion detected
            let mut func_cycle = Vec::new();
            // collect the cycle entries
            for entry in self.func_stack.iter().skip(cycle_start_idx) {
                func_cycle.push(entry.to_owned());
            }
            // add parent function name
            let cycle_parent_func = self
                .func_stack
                .get(cycle_start_idx - 1)
                .map(|s| s.to_owned())
                .ok_or_else(|| {
                    eyre::eyre!(
                        "Parent function not found for recursion in stack: {:?}",
                        self.func_stack
                    )
                })?;
            let sub_count = cur_func_ptr.iter_sub_funcs().count();
            assert!(
                sub_count == 0,
                "Function {} invocation at recursion detection should not have sub functions",
                func_name
            );
            let parent_func_ptr = cur_func_ptr.borrow().get_parent_ptr().ok_or_else(|| {
                eyre::eyre!(
                    "Parent function pointer not found for function: {}",
                    func_name
                )
            })?;

            log::info!(
                "Sub Function of {} follows:",
                parent_func_ptr.borrow().get_func_name_or_init()
            );
            let mut cnt = 0;
            for sub_func_ptr in parent_func_ptr.iter_sub_funcs() {
                cnt += 1;
                log::debug!(
                    "Function {} is a sub function of its parent: {}",
                    sub_func_ptr.borrow().get_func_name_or_init(),
                    parent_func_ptr.borrow().get_func_name_or_init()
                );
            }
            log::info!("Sub Function Count: {}", cnt);
            // create a new recur entry
            let recur_entry = RecurEntry {
                func_cycle,
                parent_func: cycle_parent_func,
            };
            self.recur_entries.insert(recur_entry);
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
        log::debug!("Cloned root length: {}", root_func_ptr.borrow().get_len());
        self.check_recur_impl(root_func_ptr)?;
        Ok(self.recur_entries.into_iter().collect())
    }
}
