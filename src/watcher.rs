use notify::{Event, RecursiveMode, Watcher, recommended_watcher};
use std::path::Path;
use std::sync::mpsc::channel;

pub fn watch(paths: Vec<&str>, mut on_event: impl FnMut(String)) {
    let (tx, rx) = channel::<notify::Result<Event>>();
    let mut watcher = recommended_watcher(tx).unwrap();

    for path in paths {
        watcher
            .watch(Path::new(path), RecursiveMode::Recursive)
            .unwrap();
    }

    println!("[Lattice-d] Watching configured paths");

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
