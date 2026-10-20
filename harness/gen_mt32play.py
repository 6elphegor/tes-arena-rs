"""The game with its music on through ARENA.BAT's own music driver:
gen_musicplay.py's cases with MT32MPU.ADV loaded (-Mmt32mpu.adv -IOM330
-IRQM2 -DMAM1) on an MPU-401 (fmmus.py's `Mpu`) instead of the FM chips.

The lines are gen_musicplay.py's, plus each case's (and the load's)
`mpu | NAME | LENGTH DIGEST`: the bytes the driver sent to the MIDI out
since the line before.

Usage: gen_mt32play.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [--dump DIR] [CASE...]
"""
import gen_musicplay

if __name__ == '__main__':
    gen_musicplay.main('mt32')
