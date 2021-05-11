use std::io::Cursor;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use rand::{thread_rng, RngCore};
use rayon::iter::IndexedParallelIterator;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;

use crate::binary_writer::{BinaryWriter, StorageMode};
use crate::fast_rand_bool::FastRandBool;
use crate::hash_entry::{Direction, HashEntry};
use crate::multi_thread_buckets::{BucketsThreadDispatcher, MultiThreadBuckets};
use crate::pipeline::Pipeline;
use crate::smart_bucket_sort::{smart_radix_sort, SortKey};
use crate::unitig_link::{UnitigFlags, UnitigIndex, UnitigLink};
use crate::vec_slice::VecSlice;

impl Pipeline {
    pub fn hashes_sorting(
        file_hashes_inputs: Vec<PathBuf>,
        output_dir: impl AsRef<Path>,
        buckets_count: usize,
    ) -> Vec<PathBuf> {
        let mut links_buckets = MultiThreadBuckets::<BinaryWriter>::new(
            buckets_count,
            &(output_dir.as_ref().join("links"), StorageMode::Plain),
        );

        file_hashes_inputs
            .par_iter()
            .enumerate()
            .for_each(|(index, input)| {
                let mut links_tmp = BucketsThreadDispatcher::new(65536, &links_buckets);

                let mut rand_bool = FastRandBool::new();

                let file = filebuffer::FileBuffer::open(input).unwrap();

                let mut reader = Cursor::new(file.deref());
                let mut vec: Vec<HashEntry> = Vec::new();

                while let Ok(value) = bincode::deserialize_from(&mut reader) {
                    vec.push(value);
                }

                struct Compare {}
                impl SortKey<HashEntry> for Compare {
                    fn get(value: &HashEntry) -> u64 {
                        value.hash
                    }
                }

                // vec.sort_unstable_by_key(|e| e.hash);
                smart_radix_sort::<_, Compare, false>(&mut vec[..], 64 - 8);

                let mut unitigs_vec = Vec::new();

                for x in vec.group_by(|a, b| a.hash == b.hash) {
                    if x.len() == 2 && x[0].direction != x[1].direction {
                        let (fw, bw) = match x[0].direction {
                            Direction::Forward => (0, 1),
                            Direction::Backward => (1, 0),
                        };

                        let (slice_fw, slice_bw) = if rand_bool.get_randbool() {
                            unitigs_vec.push(UnitigIndex::new(
                                x[bw].bucket as usize,
                                x[bw].entry as usize,
                            ));
                            (VecSlice::new(unitigs_vec.len() - 1, 1), VecSlice::EMPTY)
                        } else {
                            unitigs_vec.push(UnitigIndex::new(
                                x[fw].bucket as usize,
                                x[fw].entry as usize,
                            ));
                            (VecSlice::EMPTY, VecSlice::new(unitigs_vec.len() - 1, 1))
                        };

                        if (x[fw].bucket == 0 && x[fw].entry == 394310)
                            || (x[bw].bucket == 0 && x[bw].entry == 394310)
                        {
                            println!(
                                "Found while hashing! {:?}/{:?} {:?}/{:?} [{}/{}]",
                                x[fw].bucket,
                                x[fw].entry,
                                x[bw].bucket,
                                x[bw].entry,
                                x[fw].hash,
                                x[bw].hash
                            );
                        }

                        links_tmp.add_element(
                            x[fw].bucket as usize,
                            &unitigs_vec,
                            UnitigLink {
                                entry: x[fw].entry,
                                flags: UnitigFlags::new_direction(true),
                                entries: slice_fw,
                            },
                        );

                        links_tmp.add_element(
                            x[bw].bucket as usize,
                            &unitigs_vec,
                            UnitigLink {
                                entry: x[bw].entry,
                                flags: UnitigFlags::new_direction(false),
                                entries: slice_bw,
                            },
                        );

                        // println!(
                        //     "A: [{}]/{} B: [{}]{}",
                        //     x[0].bucket, x[0].entry, x[1].bucket, x[1].entry
                        // );
                    }
                }
                links_tmp.finalize(&unitigs_vec);
                println!("Done {}!", index);
            });
        links_buckets.finalize()
    }
}