//! Plan de pages (ADR 0002, J4) : le chemin rapide (pages de données supposées pleines) coïncide avec le plan complet
//! sur le corpus, les groupes couvrent le fichier sans trou, l'ouverture ne lit aucune page de données.
//! Sauté sans `UNIFORMAT_CORPUS`.

use uniformat_sas::Compression;
use uniformat_sas_io::{CountingSource, FileSource, OpenOptions, PagePlan, PlanMode, SasFile};
use uniformat_testkit::Corpus;

fn assert_covers(
    plan: &PagePlan,
    groups: &[uniformat_sas_io::PageGroup],
    row_count: u64,
    what: &str,
) {
    let mut next = 0u64;
    let mut rows = 0u64;
    for g in groups {
        assert_eq!(g.pages.start, next, "{what} : trou ou chevauchement");
        assert!(g.pages.end > g.pages.start, "{what} : groupe vide");
        assert_eq!(
            plan.rows_before(g.pages.start),
            Some(g.rows_before),
            "{what} : rows_before"
        );
        next = g.pages.end;
        rows += g.rows;
    }
    assert_eq!(next, plan.page_count(), "{what} : pages non couvertes");
    assert_eq!(rows, row_count, "{what} : lignes physiques");
}

#[test]
fn fast_plan_agrees_with_full_plan_on_the_corpus() {
    let Some(corpus) = Corpus::from_env() else {
        eprintln!("UNIFORMAT_CORPUS absent : test sauté");
        return;
    };
    let (mut checked, mut mismatching) = (0usize, Vec::new());
    for f in corpus.files().expect("corpus") {
        let stem = f.stem();
        let mut options = OpenOptions::default();
        if f.path.to_string_lossy().contains("datareader") {
            options.encoding_override = Some(uniformat_sas::UTF_8);
        }
        let Ok(file) = SasFile::open(FileSource::open(&f.path).expect("source"), options) else {
            continue;
        };
        let meta = file.meta();
        if meta.compression != Compression::None {
            let plan = file.page_plan(PlanMode::Full).expect("plan");
            assert!(!plan.is_splittable(), "{stem} : table compressée");
            assert_eq!(plan.split(8).len(), 1);
            continue;
        }
        let full = file.page_plan(PlanMode::Full).expect("plan complet");
        let fast = file.page_plan(PlanMode::Fast).expect("plan rapide");
        assert!(full.is_exact());
        let total = full.rows_before(full.page_count()).unwrap_or(0);
        assert_eq!(
            total, meta.row_count,
            "{stem} : lignes physiques des pages ≠ row_count"
        );
        for parts in [1usize, 4, 8] {
            assert_covers(
                &full,
                &full.split(parts),
                total,
                &format!("{stem} split {parts}"),
            );
        }
        assert_covers(
            &full,
            &full.chunk(64 * 1024),
            total,
            &format!("{stem} chunk"),
        );
        // le plan rapide est-il confirmé par chaque page ?
        let bad = (0..full.page_count()).find(|&p| {
            let found = full
                .rows_before(p + 1)
                .zip(full.rows_before(p))
                .map_or(0, |(a, b)| a - b);
            fast.check_page(p, u32::try_from(found).unwrap_or(u32::MAX))
                .is_err()
        });
        checked += 1;
        if let Some(p) = bad {
            mismatching.push(format!("{} (page {p})", f.path.display()));
        }
    }
    eprintln!(
        "{checked} fichiers non compressés ; plan rapide contredit pour {} : {mismatching:?}",
        mismatching.len()
    );
    assert!(checked >= 170);
}

/// Le plan complet d'un fichier à lignes supprimées ou à pages partielles reste exact, et le plan rapide est refusé
/// proprement (erreur typée) s'il se trompe.
#[test]
fn fast_plan_rejects_a_wrong_hypothesis() {
    let Some(corpus) = Corpus::from_env() else {
        eprintln!("UNIFORMAT_CORPUS absent : test sauté");
        return;
    };
    let path = corpus.root().join("files/taille/taille_n50000_no.sas7bdat");
    let file = SasFile::open(
        FileSource::open(&path).expect("source"),
        OpenOptions::default(),
    )
    .expect("open");
    let fast = file.page_plan(PlanMode::Fast).expect("plan");
    // une page de données qui ne porte pas rmax lignes contredit le plan
    let err = fast.check_page(3, 1).expect_err("contredit");
    assert!(err.to_string().contains("plan de pages contredit"), "{err}");
}

/// L'ouverture ne lit que l'en-tête et les pages de métadonnées : jamais une page de données.
#[test]
fn open_reads_only_header_and_metadata_pages() {
    let Some(corpus) = Corpus::from_env() else {
        eprintln!("UNIFORMAT_CORPUS absent : test sauté");
        return;
    };
    let mut checked = 0usize;
    for f in corpus.files().expect("corpus") {
        let options = OpenOptions {
            encoding_override: f
                .path
                .to_string_lossy()
                .contains("datareader")
                .then_some(uniformat_sas::UTF_8),
            ..OpenOptions::default()
        };
        let src = CountingSource::new(FileSource::open(&f.path).expect("source"));
        let Ok(file) = SasFile::open(src, options) else {
            continue;
        };
        let h = &file.meta().header;
        let scanned = file.scanned_pages();
        // pages de métadonnées = pages lues entièrement ; la dernière (page de données) n'est lue que par son en-tête
        let meta_pages = scanned
            .iter()
            .filter(|p| {
                !matches!(
                    p.kind,
                    uniformat_sas::PageKind::Data | uniformat_sas::PageKind::Compressed
                )
            })
            .count() as u64;
        let bound = u64::from(h.header_length) + meta_pages * u64::from(h.page_length);
        let read = file.source().bytes_read();
        assert!(
            h.header_length >= 552,
            "{} : en-tête trop court pour la borne",
            f.path.display()
        );
        assert!(
            read <= bound,
            "{} : {read} octets lus > HL + {meta_pages} × P = {bound}",
            f.path.display()
        );
        checked += 1;
    }
    eprintln!("{checked} fichiers ouverts sous la borne HL + pages de métadonnées × P");
    assert!(checked >= 200);
}
