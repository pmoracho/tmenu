#!/bin/bash
set -euo pipefail

# Colores para output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Función para imprimir con color
log_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_warn() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Función para hacer die
die() {
    log_error "$1"
    exit 1
}

# ============================================================================
# MAIN SCRIPT
# ============================================================================

main() {
    log_info "=== Release Script for tmenu ==="
    echo ""

    # 1. Obtener versión actual
    log_info "[1/12] Detectando versión actual..."
    CURRENT_VERSION=$(grep "^version" Cargo.toml | head -1 | sed 's/.*"\([^"]*\)".*/\1/')
    log_success "Versión actual: $CURRENT_VERSION"
    echo ""

    # 2. Verificar si se pasó una versión como argumento
    if [ $# -eq 0 ]; then
        log_warn "Uso: ./scripts/release.sh <version>"
        log_warn "Ejemplo: ./scripts/release.sh 1.0.8"
        echo ""
        log_info "Continuando con bump automático (patch)..."

        # Bump automático: patch
        NEW_VERSION=$(echo "$CURRENT_VERSION" | awk -F. '{print $1"."$2"."$3+1}')
    else
        NEW_VERSION="$1"
    fi

    log_info "[2/12] Nueva versión será: $NEW_VERSION"
    echo ""

    # 3. Validar formato de versión
    if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        die "Versión inválida: $NEW_VERSION (debe ser X.Y.Z)"
    fi
    log_success "Versión válida"
    echo ""

    # 4. Verificar rama
    log_info "[3/12] Verificando rama..."
    BRANCH=$(git rev-parse --abbrev-ref HEAD)
    if [ "$BRANCH" != "main" ] && [ "$BRANCH" != "master" ]; then
        die "No estás en main/master. Rama actual: $BRANCH"
    fi
    log_success "En rama: $BRANCH"
    echo ""

    # 5. Verificar estado limpio
    log_info "[4/12] Verificando estado de git..."
    if [ -n "$(git status --porcelain)" ]; then
        log_error "Hay cambios sin commitear:"
        git status --short
        die "Commit o descarta cambios antes de hacer release"
    fi
    log_success "Working directory limpio"
    echo ""

    # 6. Compilar español
    log_info "[5/12] Compilando versión en español..."
    if ! cargo build --release --features lang_es 2>&1 | tail -5; then
        die "Fallo en compilación español"
    fi
    log_success "Compilación español exitosa"
    echo ""

    # 7. Compilar inglés
    log_info "[6/12] Compilando versión en inglés..."
    if ! cargo build --release --no-default-features --features lang_en 2>&1 | tail -5; then
        die "Fallo en compilación inglés"
    fi
    log_success "Compilación inglés exitosa"
    echo ""

    # 8. Ejecutar tests
    log_info "[7/12] Ejecutando tests..."
    if ! cargo test --lib 2>&1 | tail -3; then
        die "Fallo en tests"
    fi
    log_success "Tests exitosos"
    echo ""

    # 9. Actualizar versión en Cargo.toml
    log_info "[8/12] Actualizando Cargo.toml..."
    sed -i.bak "s/version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml
    rm -f Cargo.toml.bak
    log_success "Versión actualizada a $NEW_VERSION"
    echo ""

    # 10. Actualizar Cargo.lock
    log_info "[9/12] Actualizando Cargo.lock..."
    cargo update --package tmenu 2>&1 | tail -2 || true
    log_success "Cargo.lock actualizado"
    echo ""

    # 11. Crear commit y tag
    log_info "[10/12] Creando commit y tag..."
    git add Cargo.toml Cargo.lock
    git commit -m "chore: release v$NEW_VERSION"
    git tag -a "v$NEW_VERSION" -m "Release version $NEW_VERSION"
    log_success "Commit y tag creados"
    echo ""

    # 12. Mostrar instrucciones
    log_info "[11/12] Resumen de cambios:"
    git log --oneline -3
    echo ""
    log_info "[12/12] ¡Listo para hacer push!"
    echo ""
    echo -e "${BLUE}Próximos pasos:${NC}"
    echo ""
    echo "1. Revisar los cambios:"
    echo "   git show"
    echo ""
    echo "2. Hacer push (dispara GitHub Actions):"
    echo "   git push origin main --tags"
    echo ""
    echo "3. Monitorear:"
    echo "   gh run watch --repo pmoracho/tmenu"
    echo ""
    echo "4. Ver release:"
    echo "   gh release view v$NEW_VERSION"
    echo ""

    # Preguntar si hacer push automáticamente
    read -p "¿Hacer push automáticamente? (s/n): " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Ss]$ ]]; then
        log_info "Haciendo push..."
        git push origin main
        git push origin "v$NEW_VERSION"
        log_success "¡Push completado! Workflow iniciado."
        echo ""
        echo "Monitorea en: https://github.com/pmoracho/tmenu/actions"
    else
        log_warn "Push pendiente. Cuando estés listo:"
        echo "  git push origin main --tags"
    fi
}

# Ejecutar
main "$@"