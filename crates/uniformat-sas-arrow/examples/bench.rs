//! Premiers chiffres de lecture (`docs/architecture.md` §6.5) : débit du driver → `RecordBatch`.
//!
//! Usage : `cargo run --release -p uniformat-sas-arrow --example bench -- <corpus> [répétitions]`
//! Sortie : une ligne TSV par cas `cas  fichier  lignes  octets  médiane_ms  Mio/s  lignes/s`.
//! Chaque cas est mesuré `répétitions` fois (20 par défaut), fichier déjà dans le cache du système.
#![allow(clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use uniformat_sas_arrow::{BatchReader, ReadOptions};
use uniformat_sas_io::{FileSource, PlanMode, SasFile};

fn open(path: &Path, ro: &ReadOptions) -> Arc<SasFile<FileSource>> {
    Arc::new(SasFile::open(FileSource::open(path).unwrap(), ro.open_options()).unwrap())
}

fn rows_of(
    reader: impl Iterator<Item = Result<arrow_array::RecordBatch, uniformat_sas_arrow::Error>>,
) -> u64 {
    reader.map(|b| b.unwrap().num_rows() as u64).sum()
}

fn report(case: &str, file: &str, rows: u64, bytes: u64, mut times: Vec<f64>) {
    times.sort_by(f64::total_cmp);
    let med = times[times.len() / 2];
    println!(
        "{case}\t{file}\t{rows}\t{bytes}\t{:.2}\t{:.1}\t{:.0}",
        med * 1e3,
        bytes as f64 / 1048576.0 / med,
        rows as f64 / med
    );
}

fn bench<F: FnMut() -> u64>(case: &str, file: &str, bytes: u64, reps: usize, mut f: F) {
    let rows = f(); // échauffement
    let times: Vec<f64> = (0..reps)
        .map(|_| {
            let t = Instant::now();
            let n = f();
            assert_eq!(n, rows);
            t.elapsed().as_secs_f64()
        })
        .collect();
    report(case, file, rows, bytes, times);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().expect("dossier du corpus"));
    let reps: usize = args.next().and_then(|r| r.parse().ok()).unwrap_or(20);
    let ro = ReadOptions::default();
    println!("cas\tfichier\tlignes\toctets\tmediane_ms\tMio_s\tlignes_s");
    for rel in [
        "files/taille/taille_n50000_no.sas7bdat",
        "files/taille/taille_n50000_char.sas7bdat",
        "files/taille/taille_n50000_binary.sas7bdat",
        "files/base/base_n3000_mix_no.sas7bdat",
        "files/taille/taille_wide300_no.sas7bdat",
        "files/taille/taille_wide5000_no.sas7bdat",
        "files/compression/compression_pat4_char.sas7bdat",
        "files/compression/compression_pat2_binary.sas7bdat",
    ] {
        let path = root.join(rel);
        if !path.exists() {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let file = open(&path, &ro);
        let bytes = std::fs::metadata(&path).unwrap().len();
        let pages = 0..file.meta().header.page_count;
        let ncols = file.meta().columns.len();
        // lecture complète
        bench("complet", &name, bytes, reps, || {
            rows_of(
                BatchReader::new(Arc::clone(&file), pages.clone(), None, None, ro.clone()).unwrap(),
            )
        });
        // même lecture, dans un fil dédié (écart éventuel dû à l'allocateur du fil principal)
        bench("complet_fil", &name, bytes, reps, || {
            let (file, pages, ro) = (Arc::clone(&file), pages.clone(), ro.clone());
            std::thread::spawn(move || {
                rows_of(BatchReader::new(file, pages, None, None, ro).unwrap())
            })
            .join()
            .unwrap()
        });
        // projection de 2 colonnes
        if ncols >= 2 {
            bench("projection2", &name, bytes, reps, || {
                rows_of(
                    BatchReader::new(
                        Arc::clone(&file),
                        pages.clone(),
                        Some(vec![0, 1]),
                        None,
                        ro.clone(),
                    )
                    .unwrap(),
                )
            });
        }
        // LIMIT 1000
        bench("limit1000", &name, bytes, reps, || {
            rows_of(
                BatchReader::new(
                    Arc::clone(&file),
                    pages.clone(),
                    None,
                    Some(1000),
                    ro.clone(),
                )
                .unwrap(),
            )
        });
        // parallèle : un fil par groupe de pages (plan complet exact)
        if file
            .page_plan(PlanMode::Fast)
            .map(|p| p.is_splittable())
            .unwrap_or(false)
        {
            for parts in [1usize, 4, 8] {
                bench(&format!("parallele{parts}"), &name, bytes, reps, || {
                    let plan = Arc::new(file.page_plan(PlanMode::Fast).unwrap());
                    let handles: Vec<_> = plan
                        .split(parts)
                        .into_iter()
                        .map(|g| {
                            let (file, plan, ro) =
                                (Arc::clone(&file), Arc::clone(&plan), ro.clone());
                            std::thread::spawn(move || {
                                rows_of(
                                    BatchReader::with_group(file, &g, plan, None, None, ro)
                                        .unwrap(),
                                )
                            })
                        })
                        .collect();
                    handles.into_iter().map(|h| h.join().unwrap()).sum()
                });
            }
        }
    }
}
