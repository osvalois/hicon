graph TD
    A[Inicio] --> B[Fase de Inicialización]
    B --> C[Asignación de Nodos a Capas]
    B --> D[Establecimiento de Factores de Redundancia]
    C --> E[Fase CCR]
    D --> E
    E --> F{Discrepancia Detectada?}
    F -->|Sí| G[Proceso de Reconciliación]
    F -->|No| H[Fase CCC]
    G --> H
    H --> I{Comunicación Entre Capas Necesaria?}
    I -->|Sí| J[Comunicación Directa Optimizada]
    I -->|No| K[Comunicación Capa por Capa]
    J --> L[Verificación de Consenso]
    K --> L
    L --> M{Consenso Alcanzado?}
    M -->|Sí| N[Actualización de Estado Global]
    M -->|No| O[Ajuste de Parámetros]
    N --> P[Checkpoint]
    O --> E
    P --> Q[Fin]