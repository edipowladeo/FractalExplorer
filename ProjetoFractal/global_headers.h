#ifndef GLOBAL_HEADERS_H
#define GLOBAL_HEADERS_H

#include "Ccoordenadas.h"  

class AAAA{};

enum retornoGUI {alterouPosicao,alterouZoom,redimensionarJanela,clique,paleta,circular};

#define largura_textura 32
#define altura_textura largura_textura

#define COLUNAS_DE_SPRITES 1200/largura_textura*3
#define LINHAS_DE_SPRITES 1000/altura_textura*3

typedef double TCoordenadas;
typedef unsigned long int TIteracoes;


constexpr auto TAMANHO_TAREFA_BACK = 1000;
constexpr auto MAX_TAREFAS_ALOCAR = 10;
constexpr auto IMAX_TAREFABACK = 1500;
constexpr auto MAX_QUEUE_TAREFAS_FRONT = 200;

//#include "opencl_host.cpp"

//OPENCL MELHOR OCM 256 , SEM OPENCL 64

#define DURACAOFRAMEDESEJADA 60

#define INTERPOLACAO 1
#define LIMIAR_ARRASTAR_PIXELS 1 // quantidade de pixels minimos que precisa arrastar para nao realizar zoom (ao quadrado)

#define COR_INFINITO 0xffFF0000
#define COR_SPRITE_INVISIVEL sf::Color(255,0,255,180)
#define COR_SPRITE_NORMAL sf::Color(0,0,0,255)

#define USAR_SHADER 1

#define SAMPLING_ITERACOES 512 //quantidade de cores entre duas iterações aparentemente + de 16 n surte efeito

#define BORDA 0

#define LIMITE_DIVERGENCIA 128

#define NUM_PALETAS 15

#define SOBREPOSICAO 10








#endif
