import type { Transition, Variants } from "motion/react";

export const spring = {
  snappy: { type: "spring", stiffness: 560, damping: 36, mass: 0.7 },
  smooth: { type: "spring", stiffness: 300, damping: 32, mass: 0.9 },
  gentle: { type: "spring", stiffness: 210, damping: 28, mass: 1 },
  lively: { type: "spring", stiffness: 420, damping: 20, mass: 0.8 },
} satisfies Record<string, Transition>;

export const ease = {
  out: [0.16, 1, 0.3, 1],
  inOut: [0.65, 0, 0.35, 1],
  soft: [0.33, 1, 0.68, 1],
} as const;

export const fade: Transition = { duration: 0.16, ease: ease.out };

export const pageVariants: Variants = {
  initial: { opacity: 0 },
  animate: { opacity: 1, transition: { duration: 0.2, ease: ease.out } },
  exit: (direction: number) => ({
    opacity: 0,
    y: direction * -6,
    transition: { duration: 0.13, ease: ease.inOut },
  }),
};

export const listItemVariants: Variants = {
  hidden: { opacity: 0, y: 8 },
  visible: (index: number) => ({
    opacity: 1,
    y: 0,
    transition: {
      ...spring.smooth,
      delay: Math.min(index, 14) * 0.022,
      opacity: { duration: 0.24, delay: Math.min(index, 14) * 0.022 },
    },
  }),
  exit: { opacity: 0, scale: 0.985, transition: { duration: 0.14 } },
};

export const dialogVariants: Variants = {
  initial: { opacity: 0, scale: 0.965, y: 10 },
  animate: { opacity: 1, scale: 1, y: 0, transition: { ...spring.gentle, opacity: { duration: 0.18 } } },
  exit: { opacity: 0, scale: 0.98, y: 6, transition: { duration: 0.13, ease: ease.inOut } },
};

export const overlayVariants: Variants = {
  initial: { opacity: 0 },
  animate: { opacity: 1, transition: { duration: 0.2, ease: ease.out } },
  exit: { opacity: 0, transition: { duration: 0.15, ease: ease.inOut } },
};
