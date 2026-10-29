"""
TDS-1 DDM downlink compression: delta encoding + variable-rate quantisation (VRQ)

Render (Manim Community Edition >= 0.17):
    manim -pql ddm_vrq_encoding.py DDMEncoding      # quick preview
    manim -pqh ddm_vrq_encoding.py DDMEncoding      # 1080p

Byte format assumed (from the decode pseudocode):
    bit 7      : flag.  1 = the NEXT byte belongs to the same pixel
                        0 = this is the LAST byte of the pixel
    bits 6..0  : 7 payload bits
    Byte0 is read first and holds the least-significant 7 bits.
    1 byte -> 7-bit, 2 bytes -> 14-bit, 3 bytes -> 21-bit signed (two's complement) delta.
    Decoder: pixel = previous_pixel + sign_extend(payload).
"""
from manim import *
import numpy as np

# =============================== CONFIG ===============================
GRID_N = 4                 # 4x4 image
N_ANIMATE = 8              # how many pixels (raster order) to animate
SHIFT = 0                  # right shift applied to the 16-bit pixel before delta coding
INITIAL_PREDICTOR = 0      # "previous pixel" for the very first pixel

# 16-bit DDM pixel values, raster order. Chosen so the first 8 pixels
# exercise 1-, 2- and 3-byte codes.
RAW = [1200, 1215, 1190, 1230, 1195, 9000, 9020, 40000,
       39990, 39950, 39980, 12000, 12010, 11990, 12020, 12005]
VALS = [v >> SHIFT for v in RAW]

MONO = "Monospace"         # change to "Courier New" etc. if this font is missing

# ----- layout constants -----
CELL = 0.30                # one bit cell
GAP = 0.20                 # gap between bytes in the bar
PITCH = 8 * CELL + GAP     # horizontal distance between byte slots
SLOTS = 5                  # bytes visible in the bar
BAR_Y = 2.85
PX = 0.95                  # image pixel size
GRID_C = np.array([-4.6, -1.0, 0.0])
PANEL_CX = 2.3             # centre x of the right-hand working panel
PANEL_LEFT = -1.8

PALETTE = ["#2b3a8c", "#1f78b4", "#1b9e77", "#9ab800", "#e0a010", "#d94a2b"]
FLAG_CONT = "#f28c28"      # MSB = 1
FLAG_END = "#3cb371"       # MSB = 0
BG = "#14161c"


# =============================== helpers ===============================
def val_color(v):
    """Map a 16-bit value to a colour along PALETTE."""
    t = min(max(v / 65535.0, 0.0), 1.0) * (len(PALETTE) - 1)
    i = min(int(t), len(PALETTE) - 2)
    a = np.array(color_to_rgb(PALETTE[i]))
    b = np.array(color_to_rgb(PALETTE[i + 1]))
    return rgb_to_color(a + (b - a) * (t - i))


def vrq_encode(delta):
    """Return (n_bytes, n_bits, payload_bits, [byte0, byte1, ...])."""
    for n in (1, 2, 3):
        bits = 7 * n
        lo, hi = -(1 << (bits - 1)), (1 << (bits - 1)) - 1
        if lo <= delta <= hi:
            break
    else:
        raise ValueError(f"delta {delta} does not fit in 3 bytes")
    payload = delta & ((1 << bits) - 1)          # two's complement
    out = []
    for k in range(n):
        chunk = (payload >> (7 * k)) & 0x7F      # Byte0 = least significant
        flag = 1 if k < n - 1 else 0             # 1 -> another byte follows
        out.append((flag << 7) | chunk)
    return n, bits, payload, out


def make_byte(value, color, px, k, last, label=None):
    """An 8-bit cell row: flag cell (MSB) coloured, payload cells tinted by pixel colour."""
    flag_col = FLAG_END if last else FLAG_CONT
    cells = VGroup()
    for i in range(8):
        bit = (value >> (7 - i)) & 1
        sq = Square(CELL)
        if i == 0:
            sq.set_stroke(flag_col, 3).set_fill(flag_col, 0.9)
        else:
            sq.set_stroke(color, 2).set_fill(color, 0.55 if bit else 0.0)
        cells.add(sq)
    cells.arrange(RIGHT, buff=0)

    texts = VGroup()
    for i, sq in enumerate(cells):
        bit = (value >> (7 - i)) & 1
        texts.add(Text(str(bit), font=MONO, font_size=18).move_to(sq))

    flag_txt = Text("MSB 0 → last byte" if last else "MSB 1 → more bytes",
                    font_size=14, color=flag_col)
    flag_txt.next_to(cells, UP, buff=0.08, aligned_edge=LEFT)
    lab_str = label if label is not None else f"px {px} · byte {k}"
    lab = Text(lab_str, font_size=14, color=GREY_B).next_to(cells, DOWN, buff=0.08)

    grp = VGroup(cells, texts, flag_txt, lab)
    if last:   # pixel boundary marker in the gap after the byte
        x = cells.get_right()[0] + GAP / 2
        y = cells.get_center()[1]
        grp.add(DashedLine([x, y + 0.45, 0], [x, y - 0.45, 0],
                           color=GREY_B, stroke_width=2))
    return grp


def make_chunk(chunk, color):
    """7 payload bits (no flag) shown in the working panel."""
    cells = VGroup()
    for i in range(7):
        bit = (chunk >> (6 - i)) & 1
        cells.add(Square(CELL).set_stroke(color, 2).set_fill(color, 0.55 if bit else 0.0))
    cells.arrange(RIGHT, buff=0)
    texts = VGroup(*[
        Text(str((chunk >> (6 - i)) & 1), font=MONO, font_size=18).move_to(sq)
        for i, sq in enumerate(cells)
    ])
    return VGroup(cells, texts)


# =============================== scene ===============================
class DDMEncoding(Scene):
    def construct(self):
        self.camera.background_color = BG
        self.cursor = None
        self.prev_box = None
        self.row_hl = None
        self.tally = None
        self.bar = []                       # byte mobjects currently in the bar
        self.panel_items = VGroup()
        self.tot_bytes = 0

        self.intro()
        self.build_layout()
        for idx in range(N_ANIMATE):
            self.encode_pixel(idx)
        self.outro()

    # ------------------------------------------------------------------
    def slot(self, j):
        return np.array([(j - SLOTS // 2) * PITCH, BAR_Y, 0.0])

    # ------------------------------------------------------------------
    def intro(self):
        title = Text("TDS-1 DDM downlink compression", font_size=40)
        sub = Text("delta encoding + variable-rate quantisation (VRQ)",
                   font_size=26, color=GREY_B)
        head = VGroup(title, sub).arrange(DOWN, buff=0.3)
        self.play(Write(title), FadeIn(sub, shift=UP * 0.2))
        self.wait(1)
        self.play(FadeOut(head))

        ex = make_byte(0b11010110, BLUE_C, 0, 0, False, label="example byte")
        ex.scale(2).move_to(UP * 1.2)
        braces = VGroup(
            Text("flag", font_size=24, color=FLAG_CONT),
            Text("7 payload bits", font_size=24, color=BLUE_C),
        )
        braces[0].next_to(ex[0][0], DOWN, buff=0.75)
        braces[1].next_to(ex[0][4], DOWN, buff=0.75)
        lines = VGroup(
            Text("MSB = 1  →  next byte belongs to the SAME pixel", font_size=26,
                 t2c={"MSB = 1": FLAG_CONT}),
            Text("MSB = 0  →  this is the LAST byte of the pixel", font_size=26,
                 t2c={"MSB = 0": FLAG_END}),
            Text("1 byte: 7 bits   ·   2 bytes: 14 bits   ·   3 bytes: 21 bits  (signed delta)",
                 font_size=22, color=GREY_B),
        ).arrange(DOWN, buff=0.25).move_to(DOWN * 2.1)
        self.play(FadeIn(ex, shift=UP * 0.2))
        self.play(FadeIn(braces))
        self.play(FadeIn(lines, lag_ratio=0.4), run_time=2)
        self.wait(2.5)
        self.play(FadeOut(VGroup(ex, braces, lines)))

    # ------------------------------------------------------------------
    def build_layout(self):
        # ---- byte-stream bar (top) ----
        win = RoundedRectangle(width=SLOTS * PITCH + 0.4, height=1.4, corner_radius=0.15,
                               stroke_color=GREY_B, stroke_width=2).move_to([0, BAR_Y, 0])
        bar_title = Text("Output byte stream  —  newest byte enters on the right",
                         font_size=20, color=GREY_B).next_to(win, UP, buff=0.08)

        # ---- image grid (bottom left) ----
        self.cells, self.cell_txt, self.cell_cols = [], [], []
        grid = VGroup()
        for idx in range(GRID_N * GRID_N):
            r, c = divmod(idx, GRID_N)
            col = val_color(VALS[idx] << SHIFT)
            sq = Square(PX).set_stroke(GREY_C, 1.5).set_fill(col, 0.25)
            sq.move_to(GRID_C + np.array([(c - (GRID_N - 1) / 2) * PX,
                                          ((GRID_N - 1) / 2 - r) * PX, 0]))
            txt = Text(str(VALS[idx]), font_size=18).move_to(sq).set_opacity(0.6)
            num = Text(str(idx), font_size=12, color=GREY_B).move_to(
                sq.get_corner(UL) + np.array([0.13, -0.13, 0]))
            self.cells.append(sq)
            self.cell_txt.append(txt)
            self.cell_cols.append(col)
            grid.add(sq, txt, num)
        grid_title = Text("DDM (4×4), raster order", font_size=20, color=GREY_B).next_to(
            grid, UP, buff=0.12)

        # colour scale under the grid
        scale = VGroup(*[
            Square(0.19).set_stroke(width=0).set_fill(val_color(int(65535 * i / 19)), 1)
            for i in range(20)
        ]).arrange(RIGHT, buff=0).next_to(grid, DOWN, buff=0.3)
        s0 = Text("0", font_size=14, color=GREY_B).next_to(scale, LEFT, buff=0.1)
        s1 = Text("65535", font_size=14, color=GREY_B).next_to(scale, RIGHT, buff=0.1)

        # ---- VRQ range table (right of panel) ----
        rows = VGroup()
        self.row_texts = []
        for n in (1, 2, 3):
            b = 7 * n
            lo, hi = -(1 << (b - 1)), (1 << (b - 1)) - 1
            t = Text(f"{n} byte{'s' if n > 1 else ' '}  {b:>2} bit  {lo}..{hi}",
                     font=MONO, font_size=18)
            self.row_texts.append(t)
            rows.add(t)
        rows.arrange(DOWN, aligned_edge=LEFT, buff=0.28)
        rows.move_to(np.array([2.2, 1.15, 0]), aligned_edge=UL)
        table_title = Text("Variable-rate quantiser (signed range)", font_size=20,
                           color=GREY_B).move_to(np.array([2.2, 1.6, 0]), aligned_edge=LEFT)

        self.play(FadeIn(VGroup(win, bar_title)), FadeIn(grid), FadeIn(grid_title),
                  FadeIn(VGroup(scale, s0, s1)), FadeIn(VGroup(table_title, rows)))
        self.wait(0.5)

    # ------------------------------------------------------------------
    def encode_pixel(self, idx):
        T = 1.0 if idx < 3 else 0.6              # speed up after the first few pixels
        r, c = divmod(idx, GRID_N)
        cell = self.cells[idx]
        val = VALS[idx]
        prev = VALS[idx - 1] if idx > 0 else INITIAL_PREDICTOR
        delta = val - prev
        n, nbits, payload, out = vrq_encode(delta)
        col = self.cell_cols[idx]

        # ---- 1. select pixel, show previous pixel ----
        anims = [cell.animate.set_fill(col, 0.95),
                 self.cell_txt[idx].animate.set_opacity(1)]
        new_cursor = SurroundingRectangle(cell, color=YELLOW, buff=0.0, stroke_width=5)
        if self.cursor is None:
            self.cursor = new_cursor
            anims.append(Create(self.cursor))
        else:
            anims.append(Transform(self.cursor, new_cursor))
        if idx > 0:
            new_prev = SurroundingRectangle(self.cells[idx - 1], color=BLUE_B,
                                            buff=0.0, stroke_width=4)
            if self.prev_box is None:
                self.prev_box = new_prev
                anims.append(Create(self.prev_box))
            else:
                anims.append(Transform(self.prev_box, new_prev))
        if len(self.panel_items) > 0:
            anims.append(FadeOut(self.panel_items))
        self.play(*anims, run_time=0.8 * T)

        # ---- 2. delta subtraction ----
        header = Text(f"Pixel {idx} · row {r} col {c}", font_size=22, color=YELLOW)
        l_val = Text(f"value = {str(val):>7}", font=MONO, font_size=26)
        l_prev = Text(f"prev  = {'-' + str(prev):>7}", font=MONO, font_size=26, color=BLUE_B)
        rule = Line(ORIGIN, RIGHT * l_val.width, stroke_width=2, color=GREY_B)
        l_delta = Text(f"delta = {delta:>+7d}", font=MONO, font_size=26, color=YELLOW)
        calc = VGroup(header, l_val, l_prev, rule, l_delta)
        if idx == 0:
            calc.add(Text("first pixel: predictor starts at 0", font_size=16, color=GREY_B))
        calc.arrange(DOWN, aligned_edge=LEFT, buff=0.2)
        calc.move_to(np.array([PANEL_LEFT, 1.7, 0]), aligned_edge=UL)

        self.play(FadeIn(header), FadeIn(l_val), FadeIn(l_prev), run_time=0.7 * T)
        self.play(Create(rule), Write(l_delta), run_time=0.8 * T)
        if idx == 0:
            self.play(FadeIn(calc[-1]), run_time=0.4 * T)

        # ---- 3. choose 1/2/3 bytes from the range table ----
        new_hl = SurroundingRectangle(self.row_texts[n - 1], color=YELLOW, buff=0.07)
        if self.row_hl is None:
            self.row_hl = new_hl
            self.play(Create(self.row_hl), run_time=0.6 * T)
        else:
            self.play(Transform(self.row_hl, new_hl), run_time=0.6 * T)

        # ---- 4. two's-complement payload split into 7-bit chunks ----
        bits_title = Text(f"{nbits}-bit two's complement  →  {n} × 7-bit payload",
                          font_size=18, color=GREY_B).move_to([PANEL_CX, -0.95, 0])
        chunks = [make_chunk((payload >> (7 * k)) & 0x7F, col) for k in range(n)]
        display = VGroup(*reversed(chunks)).arrange(RIGHT, buff=0.35)
        display.move_to([PANEL_CX, -1.6, 0])
        labels = VGroup()
        for k in range(n):
            flag = 1 if k < n - 1 else 0
            labels.add(Text(f"Byte {k}: MSB={flag}", font_size=16,
                            color=FLAG_CONT if flag else FLAG_END
                            ).next_to(chunks[k], DOWN, buff=0.12))
        sign = Text("sign", font_size=14, color=RED_B).next_to(
            chunks[n - 1][0][0], UP, buff=0.05)
        self.play(FadeIn(bits_title), FadeIn(display, shift=UP * 0.2),
                  FadeIn(labels), FadeIn(sign), run_time=0.9 * T)
        self.wait(0.4 * T)

        self.panel_items = VGroup(calc, bits_title, display, labels, sign)

        # ---- 5. emit bytes into the bar, one at a time (Byte0 first) ----
        for k, b in enumerate(out):
            last = (k == n - 1)
            hl = SurroundingRectangle(chunks[k], color=YELLOW, buff=0.07)
            self.play(Create(hl), run_time=0.3 * T)

            new_byte = make_byte(b, col, idx, k, last)
            new_byte.shift(self.slot(SLOTS - 1) - new_byte[0].get_center())

            slide, keep, dropped = [], [], []
            for old in self.bar:
                if old[0].get_center()[0] - PITCH < -(SLOTS // 2) * PITCH - 0.1:
                    slide.append(old.animate.shift(LEFT * PITCH).set_opacity(0))
                    dropped.append(old)
                else:
                    slide.append(old.animate.shift(LEFT * PITCH))
                    keep.append(old)
            self.play(*slide, FadeIn(new_byte, shift=UP * 0.9), FadeOut(hl),
                      run_time=0.9 * T)
            if dropped:
                self.remove(*dropped)
            self.bar = keep + [new_byte]
            self.wait(0.25 * T)

        # ---- 6. byte-count pips on the pixel + running tally ----
        pips = VGroup(*[Square(0.11).set_fill(WHITE, 1).set_stroke(BLACK, 1)
                        for _ in range(n)]).arrange(RIGHT, buff=0.05)
        pips.move_to(cell.get_bottom() + UP * 0.15)

        self.tot_bytes += n
        new_tally = Text(
            f"{idx + 1} px:  {16 * (idx + 1)} raw bits  →  {self.tot_bytes} bytes "
            f"= {8 * self.tot_bytes} bits",
            font_size=22).move_to([PANEL_CX, -3.4, 0])
        tally_anim = FadeIn(new_tally) if self.tally is None else Transform(self.tally, new_tally)
        if self.tally is None:
            self.tally = new_tally
        self.play(FadeIn(pips, scale=1.5), tally_anim, run_time=0.6 * T)
        self.wait(0.3 * T)

    # ------------------------------------------------------------------
    def outro(self):
        self.wait(3)
