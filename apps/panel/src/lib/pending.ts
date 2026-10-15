import { shallowRef } from 'vue'
import type { DetectedPack } from './packdetect'

/** Modpack arrastado no cadastro, publicado em seguida na aba Modpack. */
export const pendingPack = shallowRef<{ slug: string; pack: DetectedPack } | null>(null)
