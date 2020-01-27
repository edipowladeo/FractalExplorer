
#ifndef CPALETA_H
#define CPALETA_H

#include "palettes.h"
#include <iostream>
#include <stdio.h>
#include <math.h>
#include "global_headers.h"

#define MAX_TAMANHO_PALETAS 512
#define INICIAL_VELOCIDADE_CORES 1
#define INICIAL_CIRCULAR_CORES 0
#define INICIAL_PALETA 3
#define OPACO 0xff000000


//TPaletasQuickman Lista1Pal;
class Cjanela;

class Cpaleta
{
public:
    // TPaletasQuickman Lista1Pal;
    Cjanela * JanelaMae = nullptr;
    unsigned long int paleta_pre_shader[MAX_TAMANHO_PALETAS];   //tabela lookup do shader
    int offset_paleta = 0;                                      //MATIZ
    int velocidade_cores =INICIAL_VELOCIDADE_CORES;
    int paleta_atual = INICIAL_PALETA;
    int circular_cores =  INICIAL_CIRCULAR_CORES;               //BOOLEANO

    unsigned int busca_paleta(int i,int posicao);
    unsigned int paleta_interpolada(double iterac,int indice_paleta,int interpolacao);    //OBSOLETA??????
    unsigned int paleta_quickman(int indice_paleta,int iterac);
    unsigned int paleta(unsigned long int iterac_int,int indice_paleta); //retorna a cor; ja retorna o alpha correto
     int tamanho_pal(int indice_paleta);
    unsigned int paleta_precalculada_shader(unsigned long int iterac); //retorna a cor;
    unsigned int paleta_precalculada(unsigned long int iterac); //retorna a cor;
    void  precalcula_paleta(); //PRECALCULA A PALETA TODA PARA OTIMIZAR EXIBIÇÃO
};

#endif
