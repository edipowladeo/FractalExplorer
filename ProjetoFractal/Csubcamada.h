#ifndef CSUBCAMADA_H
#define CSUBCAMADA_H

#include "Ccelula.h"
#include "Cjanela.h"
#include "Ccoordenadas.h"

#include <list>

class CTarefaAlocarCelula;

#define SOBREPOSICAO 2 //sobreposicao entre dois sprites, alem de nao ter vazios [e ultil para refinamento com convolucao



class Csubcamada
{
public:
    class Cjanela * JanelaMae = nullptr;
    int log2delta; //delta = 2^(-log2delta)
    Ccelula *Celulas[COLUNAS_DE_SPRITES][LINHAS_DE_SPRITES] = {{nullptr}};
    Ccelula **PtrCelulas[COLUNAS_DE_SPRITES][LINHAS_DE_SPRITES] = {{nullptr}}; // deprecated???

    list<CtarefaFront*> TarefasFrontPresentes;
    list<CTarefaAlocarCelula*> TarefasAlocarCelulaPresentes;

    CcoordenadasPlano Coordenadas;//Coordenadas do Centro da Camada
    int offset_i = 0;
    int offset_j = 0;

    Cvetor2d coordenadas_plano_para_indice(CCoordenadas2DPlano Coordenadas_plano);
    void posiciona_celulas_tela();
    CIntervalo2DI CalculaIntervaloIJ(const CCoordenadas2DTela &CoordCentro, const sf::Vector2f &janela);
    void CalculaIntervalos();
    void atualiza_textura();
    void deletarCelula(Ccelula* Celula);
    void MarcarCelulasInuteis();

    /*
    CCoordenadas2DPlano CoordenadasTelaParaPlanoAtual(CCoordenadas2DTela CoordTela);
    CCoordenadas2DPlano CoordenadasTelaParaPlanoDesejado(CCoordenadas2DTela CoordTela);
    CCoordenadas2DTela  CoordenadasPlanoDesejadoParaTela(CCoordenadas2DPlano CoordPlano);
    CCoordenadas2DTela  CoordenadasPlanoAtualParaTela(CCoordenadas2DPlano CoordPlano);
    */

    CIntervalo2DI IntervaloAlocar;
    CIntervalo2DI IntervaloDesenhar;
    CIntervalo2DI IntervaloDesalocar;

    Csubcamada(Cjanela * Janela,long int log2delta, CcoordenadasPlano Coordenadas);
    ~Csubcamada();
} ;




#endif
