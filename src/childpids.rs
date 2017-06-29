use std::collections::{HashMap, LinkedList};

use sysinfo::{Process, System, SystemExt};

#[derive(Debug)]
pub struct Node<T> {
    value: T,
    children: Vec<Node<T>>,
}

type ProcessNode = Node<i32>;
type PidT = i32;

fn get_children(parent_pid: PidT, processes: &HashMap<PidT, Process>) -> Vec<ProcessNode> {
    let mut children = Vec::new();
    for process in processes.values() {
        if let Some(parent) = process.parent {
            if parent == parent_pid {
                children.push(ProcessNode {
                    value: process.pid,
                    children: get_children(process.pid, processes),
                })
            }
        }
    }
    children
}

fn get_child_process_tree(parent_pid: PidT) -> Vec<ProcessNode> {
    let sys = System::new();
    let processes = sys.get_process_list();
    get_children(parent_pid, processes)
}

pub fn flatten_tree_breadth_first<T>(root: Node<T>) -> Vec<T> {
    let mut queue: LinkedList<Node<T>> = LinkedList::new();
    queue.push_back(root);
    let mut children: Vec<T> = Vec::new();
    let mut i = 0;
    while let Some(current) = queue.pop_front() {
        if i > 0 {
            children.push(current.value);
        }
        for child in current.children {
            queue.push_back(child);
        }
        i += 1;
    }
    children
}

pub fn get_children_pids(parent_pid: PidT) -> Vec<PidT> {
    let children_tree = get_child_process_tree(parent_pid);
    let children = flatten_tree_breadth_first(ProcessNode {
        value: parent_pid,
        children: children_tree,
    });
    children
}
