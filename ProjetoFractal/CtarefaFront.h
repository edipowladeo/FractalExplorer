#ifndef CTARREFAFRONT_H
#define CTARREFAFRONT_H

#include "Cprograma.h"

//#include "Ccelula.h"
#include "Cjanela.h"
class Cprograma;

#include <vector>
#include <list>

using namespace std;

class Ccelula;
class Csubcamada;
class CtarefaBack;

enum Estatus{aTransferir,Transferida,processada,destruir,enviarPFinalFila};


class CtarefaFront
{
public:
    Ccelula ** Cel = nullptr; //ponteiro de ponteiro aponta para Sub->PtrCelulas[i][j];
    // assim o ponteiro vai ser nulo caso a Subcamada for apagada

    Cjanela*JanelaMae = nullptr;
    int i;
    int j;
    int resolucao;
    Cprograma *Programa = nullptr;

    list<CtarefaBack*> TarefasBackPresentes;

    CtarefaFront(Csubcamada *Sub,int in_i, int in_j, int in_resolucao,Cprograma* Programa_in);
    ~CtarefaFront();
    bool transfere();
//  int status = 0;
    Estatus status = aTransferir;
  //0 aguardando transferir
  //1 transferida aguardando resposta
  //2 tarefa processada aguardando transferir de volta
  //3 tarefa aguardando destruicao

};

#endif // CTARREFA_H
