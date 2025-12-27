use crossbeam::scope;
use fxhash::FxHashMap;
use memmap2::Mmap;
use std::collections::HashMap;
use std::fs::File;

const MEASUREMENTS_FILE: &str = "../data/measurements.txt";

#[derive(Clone, Copy)]
struct Stats {
    min: i32,
    max: i32,
    sum: i64,
    count: u32,
}
impl Stats {
    #[inline]
    fn new(value: i32) -> Self {
        Self {
            min: value,
            max: value,
            sum: value as i64,
            count: 1,
        }
    }
    #[inline]
    fn update(&mut self, value: i32) {
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.sum += value as i64;
        self.count += 1;
    }
}
#[inline]
fn temp_parser(bytes: &[u8]) -> i32 {
    let mut value = 0i32;
    let mut sign = 1;
    let mut i = 0;

    if bytes[0] == b'-' {
        sign = -1;
        i = 1;
    }
    for &b in &bytes[i..] {
        if b == b'.' {
            continue;
        }
        value = value * 10 + (b - b'0') as i32;
    }

    value * sign
}

fn main() {
    let file = File::open(MEASUREMENTS_FILE).expect("Failed to open file");
    let mmap = unsafe { Mmap::map(&file).unwrap() };
    let bytes: &[u8] = &mmap;

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let len = bytes.len();
    let chunk_size = len / threads;

    scope(|s| {
        let mut handles = Vec::with_capacity(threads);

        for t in 0..threads {
            let mut start = t * chunk_size;
            let mut end = if t == threads - 1 {
                len
            } else {
                (t + 1) * chunk_size
            };

            if start != 0 {
                while start < end && bytes[start - 1] != b'\n' {
                    start += 1;
                }
            }

            while end < len && bytes[end - 1] != b'\n' {
                end += 1;
            }

            let slice = &bytes[start..end];

            handles.push(s.spawn(move |_| {
                let mut map: HashMap<&[u8], Stats> = HashMap::new();

                let mut line_start = 0;
                let mut i = 0;

                while i < slice.len() {
                    if slice[i] == b'\n' {
                        let line = &slice[line_start..i];
                        line_start = i + 1;

                        let mut sep = 0;
                        while line[sep] != b';' {
                            sep += 1;
                        }

                        let station = &line[..sep];
                        let temp = temp_parser(&line[sep + 1..]);

                        match map.get_mut(station) {
                            Some(stats) => {
                                stats.update(temp);
                            }
                            None => {
                                map.insert(station, Stats::new(temp));
                            }
                        }
                    }
                    i += 1;
                }
                map
            }))
        }
        println!("finished processing part.");
        let mut final_map: FxHashMap<&[u8], Stats> = FxHashMap::default();

        for handle in handles {
            let local = handle.join().unwrap();
            for (station, stats) in local {
                match final_map.get_mut(station) {
                    Some(s) => {
                        s.min = s.min.min(stats.min);
                        s.max = s.max.max(stats.max);
                        s.sum += stats.sum;
                        s.count += stats.count;
                    }
                    None => {
                        final_map.insert(station, stats);
                    }
                }
            }
        }
        println!("finished merging");

        for (station, s) in final_map {
            let avg = s.sum as f64 / s.count as f64 / 10.0;
            println!(
                "{} => min {:.1}, avg {:.1}, max {:.1}",
                std::str::from_utf8(station).unwrap(),
                s.min as f64 / 10.0,
                avg,
                s.max as f64 / 10.0
            );
        }
    })
    .unwrap();
}
