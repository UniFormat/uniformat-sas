// Point d'entree de la bibliotheque partagee : delegue a l'initialisation generee par extendr.
#include <Rinternals.h>
#include <R_ext/Rdynload.h>

void R_init_uniformatsas_extendr(void *dll);

void R_init_uniformatsas(void *dll) {
    R_init_uniformatsas_extendr(dll);
    R_useDynamicSymbols(dll, FALSE);
}
