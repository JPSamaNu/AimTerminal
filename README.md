# rt

Práctica de decisión y reflejos para la terminal, pensada para ratos de espera
(mientras compila un proyecto o corren pruebas) en un panel de Windows Terminal.
Sesiones de 1 a 2 minutos, sin sonido ni animaciones, y sin consumo de CPU en reposo.

## Compilar

Requiere Rust estable. Con el toolchain GNU (`stable-x86_64-pc-windows-gnu`) o MSVC:

```bash
cargo build --release
```

El ejecutable queda en `target\release\rt.exe`. Para tenerlo en el `PATH`
(lo copia a `%USERPROFILE%\.cargo\bin`):

```bash
cargo install --path .
```

> `Cargo.toml` fija `parking_lot 0.12.3` / `parking_lot_core 0.9.10`. Las versiones
> nuevas usan `raw-dylib` y en el toolchain GNU necesitan `dlltool` con ensamblador,
> que rustup no instala.

Pruebas y lint:

```bash
cargo test
```

```bash
cargo clippy --all-targets
```

## Uso

```
rt                     menú
rt --quick             empieza una sesión de inmediato
rt --baseline          prueba base de Decisión: 30 intentos, sin adaptación
rt --mode blancos      modo Blancos (también se elige en el menú)
rt export              intentos a CSV por stdout
rt export -o out.csv   intentos a un archivo
rt --theme mono        un solo color
rt --trials 10 --fixed --timeout 1500
rt --pixel             intenta mouse en píxeles (ver abajo)
```

En cada intento aparece un punto de fijación y, tras 0.5–2 s, de 3 a 5 recuadros.
El objetivo es el bloque **relleno** (verde); los distractores son **huecos** (rojos).
La diferencia no depende solo del color: con `--theme mono` se distinguen por el relleno.
Responde con un clic sobre el objetivo o con la tecla de su número.
En el ~17% de los intentos no hay objetivo: lo correcto es no responder.

### Modo Blancos

Hay 3 recuadros verdes a la vez durante 30 s. Al hacer clic en uno, solo ese cambia de
lugar, y nunca aparece bajo el cursor. Es solo con mouse. Un clic en el vacío cuenta
como fallo. Cada clic es un intento: su tiempo se mide desde que se dibujó el cuadro
anterior, y la distancia se mide desde el cursor en ese momento hasta el blanco, así que
sirve para ver el tiempo frente a la distancia y el tamaño. El reloj se detiene en pausa.
El resumen muestra aciertos, fallos, precisión, ritmo (aciertos por segundo), mediana
entre aciertos y el mismo puntaje combinado. Se configura con `targets_count`,
`targets_secs`, `targets_w` y `targets_h`.

Teclas: `1`–`9` responder · `Esc` pausa (otra vez `Esc` termina la sesión) · `q` sale
de inmediato · en el resumen `Enter` repite y `p` abre el progreso · en el progreso
`←→` o `Tab` cambia entre decisión, baseline y blancos. En el menú, `←→` cambia el modo
y la dificultad.

El resumen compara cada métrica con la sesión anterior del mismo modo y dificultad
(verde si mejora, rojo si empeora). El progreso muestra, por métrica, el último valor,
la tendencia frente a la mediana de las 5 sesiones anteriores, el rango y una gráfica
de barras que crece en altura cuando el panel es grande.

Si sales a mitad de una sesión, se guarda marcada como incompleta y no entra al progreso.
Si la terminal pierde el foco o cambia de tamaño durante un intento, ese intento se
descarta y no cuenta; al perder el foco la sesión se pausa.

### Resultados

| resultado | cuándo |
|---|---|
| acierto | eligió el objetivo |
| error | eligió un distractor |
| omisión | se acabó el tiempo con objetivo presente |
| falsa alarma | respondió sin objetivo presente |
| rechazo correcto | no respondió sin objetivo presente |
| anticipación | respondió antes del estímulo o en menos de 120 ms |
| fallo | clic en el vacío (modo Blancos) |

Las anticipaciones no cuentan como acierto ni entran a los tiempos; se reportan aparte.
El resumen muestra mediana, media, mejor tiempo, precisión (aciertos / intentos con
objetivo), conteos y el **puntaje** combinado: eficiencia inversa = mediana de aciertos /
precisión, en ms. Menor es mejor, y así ir rápido con muchos errores no parece mejora.

Los tiempos se toman con `Instant`: inicio justo después de dibujar y vaciar el cuadro
del estímulo, fin al llegar el evento. Incluyen la latencia de la terminal, así que
sirven para compararte contigo mismo, no con otras personas ni con otros equipos.

### Dificultad

- **fija**: usa `box_w`, `box_h`, `min_boxes`–`max_boxes` y `timeout_ms` de la configuración.
- **adaptativa** (por defecto): escalera de 10 niveles. Sube tras 3 aciertos seguidos y
  baja tras un fallo (error, omisión, falsa alarma o anticipación), lo que la mantiene
  cerca del 80% de aciertos. Los rechazos correctos no la mueven. Cada nivel achica los
  recuadros, suma opciones y recorta el tiempo límite (de 2000 a 1100 ms).

Se cambia en el menú, con `--fixed` / `--adaptive` o en el archivo de configuración.

## Datos

Todo vive en `%APPDATA%\term-aim\` (`~/.config/term-aim` fuera de Windows):

- `history.jsonl`: una línea por sesión con fecha (UTC), modo, dificultad, configuración,
  si es baseline o incompleta, intentos descartados, nivel final, estadísticas e intentos.
- `config.json` (opcional): cualquier subconjunto de estas claves.

```json
{
  "mode": "decision",
  "trials": 20,
  "timeout_ms": 2000,
  "anticipation_ms": 120,
  "catch_ratio": 0.17,
  "fixation_min_ms": 500,
  "fixation_max_ms": 2000,
  "min_boxes": 3,
  "max_boxes": 5,
  "box_w": 6,
  "box_h": 3,
  "difficulty": "adaptive",
  "start_level": 0,
  "theme": "dark",
  "target_fill": "filled",
  "distractor_fill": "hollow",
  "target_color": "green",
  "distractor_color": "red",
  "pixel_mouse": false,
  "targets_count": 3,
  "targets_secs": 30,
  "targets_w": 6,
  "targets_h": 3
}
```

Los colores aceptan `green`, `red`, `blue`, `yellow`, `cyan`, `magenta`, `gray`,
`white` (en tonos apagados) o `#rrggbb`. La baseline ignora la configuración salvo
la apariencia, para que sea comparable entre semanas.

Columnas del CSV: `session_date, mode, baseline, incomplete, difficulty, trial, level,
n_boxes, target_present, target_w, target_h, cursor_dist, rt_ms, outcome, method,
anticipation`. `cursor_dist` es la distancia del cursor al centro del objetivo cuando
aparece el estímulo, en anchos de celda (la vertical cuenta doble por la forma de la
celda); queda vacía si el mouse no se movió dentro de la ventana o si no había objetivo.

## Mouse en píxeles

Con `--pixel` (o `"pixel_mouse": true`) se pide el modo SGR-Pixels (1016). Si llegan
coordenadas mayores que la cuadrícula y la terminal informa su tamaño en píxeles,
la posición del cursor se usa con precisión de subcelda; si no, todo sigue en celdas.
En Windows crossterm lee el mouse con la API de consola, que siempre entrega celdas,
así que ahí esta opción no tiene efecto. Nada depende de ella.

## Estructura

```
src/
  main.rs          argumentos y comandos
  config.rs        configuración y CLI
  term.rs          pantalla alterna, modo raw, restauración (también en panic)
  input.rs         eventos: solo Press, movimiento del mouse, foco, hora de llegada
  render.rs        pantallas y primitivas (recuadros, fijación, gráfica)
  theme.rs         paletas
  app.rs           pantallas, sesión y bucle dirigido por eventos
  modes/mod.rs     trait GameMode y tipos comunes de intento
  modes/layout.rs  colocación de recuadros sin solaparse
  modes/decision.rs  modo Decisión
  modes/targets.rs   modo Blancos
  difficulty.rs    niveles y escalera
  stats.rs         estadísticas
  history.rs       JSON Lines y CSV
```

Para añadir un modo (Reacción pura, Seguimiento), implementa `GameMode` en
`src/modes/`, añádelo a `GameKind` y créalo en `modes::create`. La sesión, la pausa,
el descarte, el reloj, las estadísticas y el historial no cambian.
