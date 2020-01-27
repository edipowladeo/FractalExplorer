
#ifndef CJANELA_H
#define CJANELA_H




#include "Cpaleta.h"
#include "CtarefaFront.h"
#include "Csubcamada.h"
#include "global_headers.h"
//#include "list"


#include <list>

#include "PropriedadesJanela.h"
#include "DebugObjects.h"

#define INICIAL_IMAX  100
#define TRANSPARENCIA 0x66000000
#define true 1
#define false 0

class Csubcamada;

class CtarefaFront;
class Ccelula;
class CDebugObjects;


struct ConfigJanela
{

    ConfigJanela();
};

class Cjanela
{
public:
    Cprograma * Programa = nullptr;
    CpropriedadesJanela Propriedades;
    CDebugObjects DebugObjects;
    map<unsigned long int, Csubcamada*> Camadas;

    list<Ccelula*> ListaCelulasVivas;

    int smooth = 0;
    uint64_t alpha = 0xff000000;

    sf::Shader shader;

    int imax = INICIAL_IMAX;

    Cpaleta Paleta;
    sf::RenderWindow * JanelaSFML = nullptr;
 


    //VARIAVEIS PARA PRECISAO ARBITRARIA
    void ResetRetangulos();
    void ToggleFullScreen();
    void AtualizarLimites();
    CCoordenadas2DTela  pivoTela;
    CCoordenadas2DPlano pivoAtual;
    CCoordenadas2DPlano pivoDesejado;
    double DeltaDesejado;
    double DeltaAtual; // DeltaPlano/DeltaTela
    int MagnificacaoMax;
    int MagnificacaoMin;

    void relatorioCamadas();
    void verificarSeInsereCamadas();

    void reordenar_celulas();
    void atualiza_texturas();
    void plota_sprites();
    void inicializar_shader();
    void ApagarCelulasInuteis();
    void MarcarCelulasInuteis();
    int NumeroDeTarefasFront();
    int NumeroDeTarefasPrepararCelula();

    bool celula_esta_dentro(Ccelula * Celula,sf::Vector2f& JanelaVerificada);//compara com janela atual
    bool celula_estara_dentro(CcoordenadasPlano Coord,sf::Vector2f& JanelaVerificada); // compara com janela desejada
    //bool ponto_esta_dentro(CCoordenadas2DTela Ponto,sf::Vector2f& Tam_janela);
    Cjanela(Cprograma * Programa);
    ~Cjanela();

    void alterarPivoTela(TCoordTela x, TCoordTela y);
    CCoordenadas2DPlano CoordenadasTelaParaPlanoAtual(CCoordenadas2DTela CoordTela);
    CCoordenadas2DPlano CoordenadasTelaParaPlanoDesejado(CCoordenadas2DTela CoordTela);
    CCoordenadas2DTela  CoordenadasPlanoDesejadoParaTela(CCoordenadas2DPlano CoordPlano);
    CCoordenadas2DTela  CoordenadasPlanoAtualParaTela(CCoordenadas2DPlano CoordPlano);
    CCoordenadas2DTela  CoordenadasPlanoAtualParaTelaDelta(CcoordenadasPlano CoordPlano);

    void cria_nova_Subcamada(unsigned long int magnificacao);
};

#endif
