use super::support::{agrees, cuboid, graph};
use crate::world_graph::node::NodeKind;
use crate::world_graph::{ChangeSet, CopyOptions, CreateOptions, WorldGraph};

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

struct Run {
    world: WorldGraph,
    random: Random,
    assemblies: Vec<String>,
    bodies: Vec<String>,
    issued: Vec<(u32, u32)>,
}

impl Run {
    fn record(&mut self, changes: ChangeSet) {
        self.issued.extend(
            changes
                .added
                .iter()
                .map(|change| (change.handle, change.generation)),
        );
    }

    fn pick(&mut self, assembly: bool) -> Option<String> {
        loop {
            let pool = if assembly {
                &mut self.assemblies
            } else {
                &mut self.bodies
            };
            if pool.is_empty() {
                return None;
            }
            let index = self.random.below(pool.len());
            if self.world.node(&pool[index]).is_ok() {
                return Some(pool[index].clone());
            }
            pool.swap_remove(index);
        }
    }

    fn refresh(&mut self) {
        let nodes = self.world.nodes.values();
        let (assemblies, bodies): (Vec<_>, Vec<_>) =
            nodes.partition(|node| node.kind == NodeKind::SystemAssembly);
        self.assemblies = assemblies.iter().map(|node| node.og_id.clone()).collect();
        self.bodies = bodies.iter().map(|node| node.og_id.clone()).collect();
    }

    fn step(&mut self) {
        let roll = self.random.below(100);
        let parent = self.pick(true).filter(|_| roll % 2 == 0);
        let body = self.pick(false);
        match (roll, body) {
            (0..=14, _) | (_, None) => {
                let options = CreateOptions {
                    parent,
                    ..CreateOptions::default()
                };
                let (id, changes) = if roll < 10 {
                    self.world.create_system_assembly(options).unwrap()
                } else {
                    self.world.create_primitive(cuboid(), options).unwrap()
                };
                if roll < 10 {
                    self.assemblies.push(id);
                } else {
                    self.bodies.push(id);
                }
                self.record(changes);
            }
            (15..=74, Some(body)) => {
                let (id, changes) = self.world.instance(&body, copy_into(parent)).unwrap();
                self.bodies.push(id);
                self.record(changes);
            }
            (75..=84, Some(body)) => {
                let (id, changes) = self.world.duplicate(&body, copy_into(parent)).unwrap();
                self.bodies.push(id);
                self.record(changes);
            }
            (85..=89, Some(body)) => {
                self.world.dispose(&body).unwrap();
            }
            (90..=95, Some(body)) => match parent {
                Some(parent) => {
                    self.world
                        .add_child(&parent, &[body], roll % 4 == 0)
                        .unwrap();
                }
                None => {
                    if let Some(parent) = self.world.parent(&body).unwrap().map(str::to_string) {
                        self.world.remove_child(&parent, &body, false).unwrap();
                    }
                }
            },
            (_, Some(body)) => {
                self.world.make_unique(&body).unwrap();
            }
        }
    }

    fn check_sample(&mut self, count: usize) {
        assert_eq!(self.world.handles.len(), self.world.nodes.len());
        for _ in 0..count {
            let (handle, generation) = self.issued[self.random.below(self.issued.len())];
            agrees(&self.world, handle, generation);
        }
    }
}

fn copy_into(parent: Option<String>) -> CopyOptions {
    CopyOptions {
        og_id: None,
        parent: parent.map(Some),
    }
}

#[test]
fn indexed_lookup_matches_a_scan_on_a_random_graph_of_ten_thousand_nodes() {
    let mut run = Run {
        world: graph(),
        random: Random(0x9e37_79b9_7f4a_7c15),
        assemblies: Vec::new(),
        bodies: Vec::new(),
        issued: Vec::new(),
    };
    let mut steps = 0;
    while run.world.node_count() < 10_000 {
        if steps == 4_000 {
            let mark = run.world.mark().unwrap();
            for _ in 0..600 {
                run.step();
            }
            run.check_sample(200);
            run.world.rollback(mark).unwrap();
            run.world.release(mark).unwrap();
            run.refresh();
        }
        run.step();
        steps += 1;
        if steps % 1_000 == 0 {
            run.check_sample(200);
        }
    }
    let issued = run.issued.clone();
    for (handle, generation) in issued {
        agrees(&run.world, handle, generation);
        agrees(&run.world, handle, generation.wrapping_add(1));
    }
    let live = run.world.nodes.values().count();
    assert_eq!(run.world.handles.len(), live);
    assert!(live >= 10_000);
    assert!(run.issued.len() > live);
}
