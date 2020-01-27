#ifndef Cprograma_H
#define Cprograma_H

#include <SFML/Graphics.hpp>
#include <vector>


#include "CCronometro.h"
#include "CtarefaBack.h"
#include "CTarefaAlocarCelula.h"

#include <list>


class CTarefaAlocarCelula;
class CtarefaFront;
class CtarefaBack;
//#include "CtarefaFront.h"

using namespace std;

class Cprograma
{
public:
//    THost * HostMAIN;

    sf::Clock relogio_refresh;
    sf::Clock relogio_clique;
    sf::Time tempo_refresh;
    sf::Time Tempo_clique;              


    //vector<CCronometro> CCronometroAlocar;
    CCronometro CronometroAlocarCelula;
    CCronometro CronometroCriarTextura; 
    CCronometro CronometroPrepararCelula;
    
    CCronometro CronometroTarefas;

    CCronometro Cronometro1;
CCronometro Cronometro2;
CCronometro Cronometro3;
CCronometro Cronometro4;
CCronometro Cronometro5;


float TaxaDeQuadrosDesejada; //Unidade: 1/s
int TempoMinDedicadoAProcessamento; //unidade: Microssegundo


    //list<CtarefaFront*> TarefasFrontPresentes;
    list<CtarefaBack*> TarefasBackPresentes;


  void   criarLink(CtarefaFront* TarefaFront, CtarefaBack* TarefaBack);

void     apagar_todas_tarefas_livres_front();


void  atualiza_lista_tarefas();
};

#endif
