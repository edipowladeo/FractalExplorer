#ifndef PALETTES_H

#define PALETTES_H
#define QTDE_PALETAS_QUICKMAN 14

extern unsigned pal_0[];
extern unsigned pal_1[];
extern unsigned pal_2[];
extern unsigned pal_3[];
extern unsigned pal_4[];
extern unsigned pal_5[];
extern unsigned pal_6[];
extern unsigned pal_7[];
extern unsigned pal_8[];
extern unsigned pal_9[];
extern unsigned pal_10[];
extern unsigned pal_11[];
extern unsigned pal_12[];
extern unsigned pal_13[];
extern unsigned pal_14[];
extern unsigned pal_15[];
extern unsigned pal_16[];

class CpaletasQuickman
{
public:
    unsigned int * ptrpal[QTDE_PALETAS_QUICKMAN];
    int tamanho[QTDE_PALETAS_QUICKMAN];

    CpaletasQuickman();
};

extern CpaletasQuickman Lista1Pal;
#endif
