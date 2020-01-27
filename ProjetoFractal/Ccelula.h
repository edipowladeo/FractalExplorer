#ifndef CCELULA_H
#define CCELULA_H

#include <SFML/Graphics.hpp>

#include "Ccoordenadas.h"
#include "Cpaleta.h"
#include "Cjanela.h"
#include "CtarefaFront.h"

#include <list>




#define LINHAS_ADICIONAIS_TEXTURA ceil((float)MAX_TAMANHO_PALETAS/largura_textura) //linhas de textura para incluir a paleta quando shader estiver ligado

#include "Ccelula.h"
#include "global_headers.h"

class CtarefaBack;

struct CDadosCelula
{
public :
    TCoordenadas C_R;
    TCoordenadas C_I;
    TCoordenadas Z_R;
    TCoordenadas Z_I;
    // TCoordenadas C_R;
    // TCoordenadas C_I;
    TIteracoes   IT;
//    CDadosCelula()
//    {
//        cout <<"\nDadosONSTRUC";
//    }
};


struct CDadosCelulaTamEstatico
{
public :
    TCoordenadas C_R[largura_textura*altura_textura];
    TCoordenadas C_I[largura_textura*altura_textura];
    TCoordenadas Z_R[largura_textura*altura_textura];
    TCoordenadas Z_I[largura_textura*altura_textura];
    TIteracoes   IT[largura_textura*altura_textura];

};

class Ccelula
{
public:
//   mutex Trava;
// friend class teste;
    class Csubcamada *SubcamadaMae = nullptr;
    class Cjanela *JanelaMae = nullptr;
    sf::Sprite sprite;
    sf::Texture textura;



/*    TCoordenadas C_R[largura_textura*altura_textura];
    TCoordenadas C_I[largura_textura*altura_textura];
    TCoordenadas Z_R[largura_textura*altura_textura];
    TCoordenadas Z_I[largura_textura*altura_textura];
*/
    //TCoordenadas C_R[largura_textura*altura_textura];
    //TCoordenadas C_I[largura_textura*altura_textura];
    TIteracoes   IT[largura_textura * altura_textura] = { 0 };

    bool pixel_aberto[largura_textura * altura_textura] = { 0 };
    
    vector<CDadosCelula> DadosCelula;// = nullptr;


    CtarefaFront* TarefaFrontDaCelula = nullptr;
    list<CtarefaBack*> TarefasBackPresentes;

    bool esperandoTarefa = false;
    bool marcadaParaDestruicao = false;
    bool UsandoShader;

    unsigned long int matriz_iteracoes[largura_textura][altura_textura] = { {0} };          //matriz com as iteracoes float
    //float IT[largura_textura * altura_textura] = { 0 };
    unsigned long int bitmap[largura_textura * altura_textura + MAX_TAMANHO_PALETAS] = { 0 };       //matriz cores + sobra para PALETA do shader;

    sf::Uint8 *bitmap_char;
    CcoordenadasPlano coordenadas_canto; //coordenadas iniciais
    // int resolucao_base;
    int resolucao_calculada=1;
    //float resolucao_aparente = RESOLUCAO_MAX*2; //tamnho do detalhe na tela em pixels.

    bool EstaProntaParaExibir = false; //Atualizada por CtarefaBack::transfere_para_Celulas()
    bool EstaDentroDaJanelaVisivel = false; //Atualizada por CJanela::plota_sprites();
    bool MatrizCPopulada = false; //Atualizada por CtarefaFront::transfere();

    Ccelula(Csubcamada * Sub,CcoordenadasPlano Coordenadas);
    ~Ccelula();
    void  marcarParaDestruicao();
    void  textura_matriz_int_res_arbitr() ;//MEDIR VELOCIDADE E OTIMIZAR;
    void  textura_matriz_float_res_arbitr(); //MEDIR VELOCIDADE E OTIMIZAR;
    void  textura_shader_matriz_float_res_arbitr(); //MEDIR VELOCIDADE E OTIMIZAR;
    void  textura_shader_matriz_float_res1(); //MEDIR VELOCIDADE E OTIMIZAR;
    void  textura_matriz_float_res1(); //MEDIR VELOCIDADE E OTIMIZAR;
    void atualiza_textura();
    void AlocarVetoresDeDados(); 
    void ReadSomething();
} ;

#endif // CCELULA_H
