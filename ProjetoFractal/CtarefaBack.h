#ifndef CTARREFABACK_H
#define CTARREFABACK_H

class Cprograma;
class CtarefaFront;

#include "Cjanela.h"
#include "global_headers.h"
#include "CFuncoes.h"

using namespace std;

struct Tarefas_presentes{
CtarefaFront * Ptr = nullptr;
int indice_inicial = 0;
int indice_final = 0;
};


class CtarefaBack{
    public:
  Cjanela * JanelaMae = nullptr;
  Cprograma * Programa = nullptr;
  static const int tamanho = TAMANHO_TAREFA_BACK;// 32000 leva 250 ms // 3200 leva 45 ms
  TIteracoes i_max = IMAX_TAREFABACK;
  int indice=0;
  bool tarefa_fechada = false;

    TCoordenadas C_R[tamanho];
    TCoordenadas C_I[tamanho];
    TCoordenadas Z_R[tamanho];
    TCoordenadas Z_I[tamanho];
    TIteracoes   IT[tamanho];
    int indice_na_origem[tamanho];
    vector<CtarefaFront*> TarefasFrontPresentes;


    //mudar para struct
//    pixels da tareda TarefasFrontPresentes[i] vao de indice_inicial[i] até indice_final[i];
    vector<int> indice_inicial;
    vector<int> indice_final;//indices que acabam as tarefas

CtarefaBack(Cprograma* Programa_in);
~CtarefaBack();
void report();
void executa_cpu_simples();
void executa();
void transfere_para_Celulas();

};
#endif
