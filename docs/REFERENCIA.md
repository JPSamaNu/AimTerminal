# rt: referencia

Detalle de cada modo, las métricas, la dificultad, la configuración y los datos. La guía
corta está en el [README](../README.md).

- [Medición y resultados](#medición-y-resultados)
- [Modos](#modos)
- [Dificultad](#dificultad)
- [Calentamiento, baselines y plan](#calentamiento-baselines-y-plan)
- [Análisis](#análisis)
- [Apariencia](#apariencia)
- [Datos](#datos)
- [Configuración](#configuración)
- [CSV](#csv)
- [Mouse en píxeles](#mouse-en-píxeles)
- [Compilar y desarrollar](#compilar-y-desarrollar)

## Medición y resultados

Los tiempos se toman con `Instant`. El inicio se marca justo después de dibujar y vaciar
el cuadro del estímulo, y el fin al llegar el evento de entrada. Incluyen la latencia de
la terminal, así que sirven para compararte contigo mismo, no con otras personas ni con
otros equipos. Los efectos visuales se dibujan después y no tocan la medición.

| resultado | cuándo |
|---|---|
| acierto | eligió el objetivo (o un "go" a tiempo) |
| error | eligió un distractor |
| omisión | se acabó el tiempo con objetivo presente |
| falsa alarma / comisión | respondió sin objetivo, o a un "no-go" |
| rechazo correcto | no respondió sin objetivo, o a un "no-go" |
| anticipación | respondió antes del estímulo o en menos de 120 ms |
| fallo | clic en el vacío (blancos, flick); en seguimiento, un segundo con menos del 50% encima |

Las anticipaciones no cuentan como acierto ni entran a los tiempos; se reportan aparte.

- **Precisión:** aciertos ÷ intentos con objetivo.
- **Puntaje** (eficiencia inversa): mediana de los aciertos ÷ precisión, en ms. Menor es
  mejor; ir rápido con muchos errores no parece mejora.
- **Rendimiento** (ley de Fitts), en blancos y flick: para cada acierto,
  `log2(distancia / ancho + 1)` bits divididos entre el tiempo, en bits por segundo.
  Junta distancia, tamaño y velocidad en un número, así que compara sesiones aunque los
  blancos salgan a distancias o tamaños distintos. Mayor es mejor.
- **Encima** (seguimiento): fracción del tiempo con el cursor sobre el blanco.
- **Comisiones** (go/no-go): respuestas a "no-go" ÷ intentos "no-go".

El resumen compara cada métrica con la sesión anterior comparable (mismo modo, misma
dificultad y, en fija, mismo nivel): verde si mejora, rojo si empeora.

Si sales a mitad de una sesión, se guarda marcada como incompleta y no entra al
progreso. Si la terminal pierde el foco o cambia de tamaño durante un intento, ese
intento se descarta; al perder el foco la sesión se pausa.

## Modos

### Decisión

Un punto de fijación y, tras 0.5–2 s, de 3 a 5 recuadros. El objetivo es el **relleno**;
los distractores son **huecos**. La diferencia no depende solo del color. Se responde
con un clic o con la tecla del número. En el ~17% de los intentos no hay objetivo y lo
correcto es no responder. Tiempo límite: 2 s en fija.

### Blancos

3 recuadros a la vez durante 30 s. Al hacer clic en uno, solo ese cambia de lugar, y
nunca bajo el cursor. Solo mouse; un clic en el vacío es un fallo. El tiempo de cada
clic se mide desde el cuadro anterior y la distancia desde el cursor en ese momento. El
resumen muestra aciertos, fallos, precisión, ritmo, mediana entre aciertos, rendimiento,
puntaje y la mediana de los aciertos **cerca del centro** frente a los de **lejos**.

### Flick

Un solo blanco lejos del cursor; las 8 direcciones salen repartidas por igual. Un clic
fuera es un fallo y el blanco se queda (no cuenta para terminar). Si no lo tocas en 3 s
es una omisión y aparece otro. Es el modo que mejor alimenta el análisis por dirección.

### Seguimiento

El recuadro avanza, rebota en los bordes y cambia de rumbo cada tanto. Cada segundo se
guarda la fracción del tiempo con el cursor encima y la distancia media. Es el único modo
que redibuja seguido (unas 30 veces por segundo) mientras juegas.

### Reacción y Go/No-Go

Un recuadro en el centro tras 0.8–2.5 s. Se responde con espacio, un número o un clic.
En reacción hay 1 s para responder. En go/no-go, el 25% de los intentos son "no-go"
(hueco) y responder ahí es una comisión; el tiempo para responder depende del nivel.

## Dificultad

En el menú, `dificultad` (←→) recorre **fácil, normal, difícil, experto y adaptativa**
en blancos, flick, seguimiento y go/no-go, cada uno con su propia configuración.
Decisión alterna fija y adaptativa. Reacción no tiene dificultad.

**Adaptativa** es una escalera de 10 niveles (0 a 9): 3 éxitos seguidos suben un nivel y
un fallo baja uno, lo que mantiene el rendimiento cerca del 80%. Cada sesión empieza un
nivel por debajo del nivel final de la anterior. La barra de estado muestra el nivel.

| modo | éxito | fallo | sin efecto (corta la racha) |
|---|---|---|---|
| decisión, go/no-go | acierto | error, omisión, falsa alarma, anticipación | rechazo correcto (no la corta) |
| blancos | acierto en ≤ 900 ms (`targets_adapt_ms`) | clic en el vacío, acierto > 1.5× ese tiempo | acierto algo lento |
| flick | acierto en ≤ 850 ms (`flick_adapt_ms`) | fallo, omisión, acierto > 1.5× | acierto algo lento |
| seguimiento | segundo con ≥ 80% encima | segundo con < 50% | lo de en medio |

### Decisión

Fija usa `box_w`, `box_h`, `min_boxes`–`max_boxes` y `timeout_ms`. En adaptativa, cada
nivel achica los recuadros, suma opciones y recorta el tiempo límite (2000 a 1100 ms).

### Blancos: distancia al centro

La distancia de cada blanco al centro se sortea como `u^e`, con `u` al azar entre 0 y 1
y un exponente que baja con el nivel (2.5 en el 0, 0.35 en el 9).

| dificultad | nivel | distancia media al centro |
|---|---|---|
| fácil | 1 | ~30% del radio |
| normal | 3 | ~40% |
| difícil | 6 | ~60% |
| experto | 9 | ~75% |

El blanco tocado se recoloca con el nivel ya actualizado. Cada intento guarda su radio
(0 centro, 1 borde).

### Flick: tamaño y distancia

| dificultad | nivel | blanco | distancia (parte del rango) |
|---|---|---|---|
| fácil | 1 | 6x3 | 5–45% |
| normal | 3 | 5x2 | 15–65% |
| difícil | 6 | 3x1 | 40–90% |
| experto | 9 | 2x1 | 60–100% |

El rango va de 8 celdas a un 60% de la diagonal del panel. Si en una dirección no cabe
la distancia del nivel, el blanco se acerca lo justo.

### Seguimiento: velocidad, tamaño y giros

| dificultad | velocidad | blanco | cambia de rumbo cada |
|---|---|---|---|
| fácil | 8 celdas/s | 7x3 | 1.3–2.6 s |
| normal | 12 | 6x3 | 0.9–2.0 s |
| difícil | 19 | 4x2 | 0.6–1.4 s |
| experto | 30 | 3x1 | 0.3–0.8 s |

El nivel cambia en vivo, sin reiniciar el recuadro.

### Go/No-Go: tiempo para responder

| dificultad | ventana |
|---|---|
| fácil | 1.1 s |
| normal | 900 ms |
| difícil | 620 ms |
| experto | 420 ms |

## Calentamiento, baselines y plan

### Calentamiento

Los primeros 3 intentos de cada sesión (`warmup_trials` o `--warmup N`; 0 lo quita) se
juegan y se guardan marcados, pero no entran a las estadísticas, al análisis ni a la
cuenta para terminar. En blancos y seguimiento el reloj arranca al acabar. La escalera
adaptativa sí se mueve durante el calentamiento. En decisión y go/no-go el calentamiento
siempre tiene objetivo, para no alterar la proporción de intentos sin objetivo.

### Baselines

Pruebas siempre iguales para comparar semana a semana:

- **Decisión:** 30 intentos con la configuración por defecto y dificultad fija.
- **Flick:** 30 blancos en nivel 4 fijo, 3 s por blanco.

`baseline` (o `rt --baseline`) corre la de flick si el modo elegido es flick, y la de
decisión en los demás. Solo toman de tu configuración la apariencia y el calentamiento.
Cada una tiene su vista en el progreso.

### Plan de entrenamiento

`plan de hoy` en el menú (o `rt --plan`) arma 3 bloques cortos (unos 3 minutos):

- Cada modo recibe una prioridad. Sube si tu última sesión muestra una debilidad clara:
  precisión menor al 80% en decisión, comisiones mayores al 20% en go/no-go, menos del
  60% encima en seguimiento, menos del 85% de acierto en blancos, o una dirección con
  menos del 90% de acierto en el análisis de puntería. También sube si nunca jugaste el
  modo o si hace una semana que no. Se toman los 3 de mayor prioridad.
- Si fallas más hacia alguna dirección, el bloque de flick hace salir los blancos el
  doble de veces hacia tus 2 direcciones más débiles.
- Bloques: decisión 15 intentos, flick 20, go/no-go 20, reacción 15, blancos y
  seguimiento 20 s, con tu dificultad. Solo el primero calienta completo.
- `Enter` pasa al siguiente bloque y `Esc` deja el plan. Al final hay una lista con la
  métrica principal de cada bloque.

Cada bloque se guarda como sesión marcada `plan_block`; solo se compara con otros
bloques, porque son más cortos.

## Análisis

### Puntería por dirección

`a` en el resumen o `puntería` en el menú analiza las últimas 30 sesiones con mouse
(decisión, blancos y flick). Con la posición del cursor al aparecer el blanco, el centro
del blanco y el punto del clic, muestra para cada dirección (→ ↗ ↑ ↖ ← ↙ ↓ ↘):

- acierto y mediana de tiempo, en una rosa (verde ≥85%, rojo <70%);
- **pasarse:** error a lo largo del movimiento; positivo es pasarse, negativo quedarse corto;
- **lateral:** error de lado; positivo es a la derecha de la trayectoria;
- conclusiones en palabras: hacia dónde fallas más, dónde eres más lento y tus sesgos.

Las distancias van en anchos de celda, con la vertical contada doble (una celda es el
doble de alta que de ancha). Con mouse en celdas la resolución es de una celda, así que
los sesgos menores de 0.3 se ignoran.

### Progreso

Una vista por modo, más las baselines de decisión y flick (`←→`). Por métrica: último
valor, tendencia frente a la mediana de las 5 anteriores, rango y gráfica de barras.

- `d` filtra por dificultad: todas → cada una que jugaste → todas.
- `w` alterna por sesión y por semana (lunes a domingo, UTC). Cada semana usa la mediana
  de sus sesiones, así un día atípico no mueve la tendencia, y dice cuántas tuvo (`n=5`).

## Apariencia

**Camuflaje (`matrix`, por defecto, y `cyber`).** El fondo muestra salida falsa y tenue
de `cargo`, pruebas, logs, `git log`, volcados hex y código, que avanza una línea cada
1–3 s (`"ambient_motion": false` la deja quieta durante el juego). El prompt imita al
tuyo (`PS C:\ruta>`; en paneles estrechos, solo la última carpeta). El menú parece un
`rt --help`, la cuenta regresiva un `cargo run`, la barra de estado el progreso de
`cargo`, la pausa un proceso detenido, el resumen la salida de `cargo test` y el
progreso un `git log`. El objetivo es un bloque de código "seleccionado" y los
distractores un marco fino (`┌──┐`). La fijación es un cursor `_`. Usa el fondo de tu
terminal.

**Recuadros (`dark`).** Fondo degradado con una rejilla de puntos tenue, paneles
redondeados y cifras grandes de medio bloque cuando caben.

**`mono`:** un solo color. **`--plain`:** sin fondo ni efectos.

Cada respuesta deja un efecto de ~380 ms: anillo y destello en un acierto, sombreado en
un error, contorno donde estaba el objetivo en una omisión, aspa en un clic en el vacío.
Solo piden un cuadro cada 60 ms mientras duran.

Funciona desde 40x12 y se adapta al tamaño; en paneles pequeños oculta lo opcional.

## Datos

### Dónde se guardan

Por defecto en `%APPDATA%\term-aim\` (`~/.config/term-aim` fuera de Windows). Prioridad,
de mayor a menor:

1. `--data CARPETA` (solo esa ejecución);
2. la variable de entorno `RT_DATA`;
3. la carpeta guardada con `rt data set CARPETA`;
4. una carpeta `rt-data` junto al ejecutable (modo portátil: crea `bin\rt-data` y
   `bin\rt.exe` la usa sola);
5. la de por defecto.

`rt data` dice qué carpeta se usa y por qué. `rt data set CARPETA` fusiona en ella el
historial actual (sin duplicar sesiones, ordenado por fecha), copia la configuración si
allá no hay una y la recuerda; no borra la anterior. `rt data reset` vuelve a la de por
defecto.

### Archivos

- `history.jsonl`: una línea por sesión con fecha (UTC), modo, dificultad,
  configuración, si es baseline o incompleta, descartados, nivel final, estadísticas e
  intentos.
- `config.json` (opcional): cualquier subconjunto de las claves de abajo.

## Configuración

```json
{
  "mode": "decision",
  "theme": "matrix",
  "background": "grid",
  "effects": true,
  "ambient_motion": true,
  "warmup_trials": 3,
  "anticipation_ms": 120,

  "trials": 20,
  "timeout_ms": 2000,
  "catch_ratio": 0.17,
  "fixation_min_ms": 500,
  "fixation_max_ms": 2000,
  "min_boxes": 3,
  "max_boxes": 5,
  "box_w": 6,
  "box_h": 3,
  "difficulty": "adaptive",
  "start_level": 0,
  "target_fill": "filled",
  "distractor_fill": "hollow",
  "target_color": "green",
  "distractor_color": "red",

  "targets_count": 3,
  "targets_secs": 30,
  "targets_w": 6,
  "targets_h": 3,
  "targets_difficulty": "adaptive",
  "targets_level": 3,
  "targets_adapt_ms": 900,

  "flick_trials": 30,
  "flick_timeout_ms": 3000,
  "flick_difficulty": "adaptive",
  "flick_level": 3,
  "flick_adapt_ms": 850,

  "tracking_secs": 30,
  "tracking_difficulty": "adaptive",
  "tracking_level": 3,

  "reaction_trials": 20,
  "go_timeout_ms": 1000,
  "gonogo_trials": 30,
  "nogo_ratio": 0.25,
  "gonogo_difficulty": "adaptive",
  "gonogo_level": 3,

  "pixel_mouse": false,
  "cell_px": null
}
```

- `mode`: `decision`, `targets`, `flick`, `tracking`, `reaction`, `gonogo`.
- `theme`: `matrix`, `cyber`, `dark`, `mono`. `background`: `grid`, `plain`, `none`.
- `*_difficulty`: `adaptive` o `fixed`; `*_level`: 0 a 9 (en fija).
- `target_color` y `distractor_color` solo se usan con `dark`: `green`, `red`, `blue`,
  `yellow`, `cyan`, `magenta`, `gray`, `white` (en tonos apagados) o `#rrggbb`.
- `cell_px`: `[ancho, alto]` de una celda en píxeles, para `--pixel` en terminales que no
  lo informan.

Opciones de línea de comandos: `rt --help`.

## CSV

`rt export` (o `rt export -o archivo.csv`) saca un intento por fila:

`session_date, mode, baseline, incomplete, difficulty, trial, level, n_boxes,
target_present, target_w, target_h, cursor_dist, rt_ms, outcome, method, anticipation,
start_x, start_y, target_x, target_y, click_x, click_y, direction, overshoot, lateral,
score, radius, warmup`

- `cursor_dist`: distancia del cursor al centro del objetivo al aparecer el estímulo, en
  anchos de celda (vertical doble). Vacía si el mouse no se movió en la ventana.
- `start_*`: cursor al aparecer el blanco; `target_*`: su centro (en un clic fallido, el
  del blanco más cercano); `click_*`: el clic.
- `direction`, `overshoot`, `lateral`: dirección y errores del análisis de puntería.
- `score`: fracción de tiempo encima (seguimiento). `radius`: distancia al centro
  (blancos, 0 a 1). `warmup`: intento de calentamiento (fíltralo para analizar).

## Mouse en píxeles

`--pixel` (o `"pixel_mouse": true`) pide el modo SGR-Pixels (DECSET 1016). Si llegan
coordenadas mayores que la cuadrícula y se conoce el tamaño de la celda en píxeles, el
cursor se usa con precisión de subcelda y la barra de estado muestra `px`; si no, todo
sigue en celdas. Nada depende de esto.

Qué hace falta para que funcione:

1. **Una terminal que implemente 1016.** Windows Terminal **no** lo implementa. Sí lo
   hacen xterm, mintty (la de Git Bash y wsltty), foot, kitty, Konsole, Ghostty, contour
   e iTerm2. Para WezTerm las fuentes se contradicen.
2. **Conocer el tamaño de la celda en píxeles.** En Linux y macOS se lee de la terminal.
   En Windows crossterm no puede preguntarlo; ahí se indica a mano con `cell_px`.

Caminos posibles, del más al menos confiable:

- **WSL con wsltty (mintty):** compilar o descargar la versión de Linux (artefacto
  `rt-linux` del CI) y correrla dentro de WSL en wsltty. Ahí la entrada llega como
  secuencias de terminal y el tamaño en píxeles se lee solo.
- **Linux o macOS** con kitty, foot, Konsole, Ghostty o xterm.
- **`rt.exe` nativo en mintty (Git Bash):** bajo ConPTY, un reporte en píxeles llega como
  un evento de celda con valores en píxeles, que `rt` detecta; hay que poner `cell_px`
  con el tamaño de la celda de tu fuente. Depende de que ConPTY deje pasar la petición
  1016 a la terminal, lo que no está confirmado.

Ninguno de estos caminos se probó de punta a punta.

## Compilar y desarrollar

```bash
cargo build --release
```

```bash
cargo install --path .
```

> `Cargo.toml` fija `parking_lot 0.12.3` y `parking_lot_core 0.9.10`. Las versiones
> nuevas usan `raw-dylib` y en el toolchain GNU necesitan `dlltool` con ensamblador, que
> rustup no instala.

`bin/rt.exe` se actualiza en cada commit; `target/` (~700 MB) queda fuera de git. El CI
(`.github/workflows/ci.yml`) corre clippy sin warnings, las pruebas y la compilación
release en Windows y Linux en cada push, y deja los ejecutables como artefactos.

```
src/
  main.rs            argumentos y comandos
  config.rs          configuración y CLI
  datadir.rs         carpeta de datos y rt data
  term.rs            pantalla alterna, modo raw, restauración (también en panic)
  input.rs           eventos: solo Press, movimiento del mouse, foco, píxeles
  app.rs             pantallas, sesión, plan y bucle dirigido por eventos
  render.rs          pantallas del estilo de recuadros y primitivas compartidas
  render_code.rs     pantallas del estilo de código (camuflaje)
  ambient.rs         salida falsa de fondo
  theme.rs           paletas
  report.rs          qué métricas muestra cada modo, filas del progreso
  plan.rs            plan de entrenamiento
  aim.rs             puntería por dirección
  difficulty.rs      niveles y escalera
  stats.rs           estadísticas
  history.rs         JSON Lines, CSV y fechas
  modes/mod.rs       trait GameMode y tipos de intento
  modes/layout.rs    colocación de recuadros
  modes/decision.rs  decisión
  modes/targets.rs   blancos
  modes/flick.rs     flick
  modes/tracking.rs  seguimiento
  modes/single.rs    reacción y go/no-go
```

Para añadir un modo, implementa `GameMode` en `src/modes/`, añádelo a `GameKind` y
créalo en `modes::create`. La sesión, la pausa, el descarte, el reloj, el calentamiento,
las estadísticas y el historial no cambian.
