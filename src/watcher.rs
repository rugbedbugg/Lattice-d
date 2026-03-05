use notify::{Watcher, RecursiveMode, recommended_watcher, Event};
use std::sync::mpsc::channel;
use std::path::Path;


pub fn watch(
        paths: Vec<&str>,
        mut on_event: impl FnMut(String)
    ) {
    let (tx, rx) = channel::<notify::Result<Event>>();
    let mut watcher = recommended_watcher(tx).unwrap();

    for path in paths {
        watcher.watch(Path::new(path), RecursiveMode::Recursive).unwrap();
    }

    for res in rx {
        match res {
            Ok(event) => {
                let entry = format!("{:?} | {:?}", event.kind, event.paths);
                on_event(entry);
            }
            Err(e) => eprintln!("[Lattice-d] Watch error: {e}"),
        }
    }
}
