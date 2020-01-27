#include "CtarefaFront.h"

// transfere está criando uma referencia desnecessária
// ee diz que a tarefa back recebe uma tarefa front de tamanho até tamanho, ou seja um total de zero pixels
//deve funcionar assim, mas favor limpar

using namespace std;

CtarefaFront::CtarefaFront(Csubcamada *Sub,int in_i, int in_j, int in_resolucao,Cprograma* Programa_in)
{
    JanelaMae = Sub->JanelaMae;
    Programa = Programa_in;

//    Programa->TarefasFrontPresentes.push_back(this);
    Sub->TarefasFrontPresentes.push_back(this);
    i = in_i;
    j = in_j;

    resolucao = in_resolucao;
    Cel = Sub->PtrCelulas[i][j];


    /*  for (auto Itr = Programa->TarefasFrontPresentes.begin();Itr!=Programa->TarefasFrontPresentes.end();Itr++)
      {
          //sort tarefas by resolucao
      }*/

    stringstream SS_endereco;
    SS_endereco << this;
    string endereco = SS_endereco.str();
//       JanelaMae->listBox_TarefasFrontPresentes->addItem(SS_endereco.str(),endereco);
//atualiza_lista();
    // transfere();
}

CtarefaFront::~CtarefaFront()
{
    stringstream SS_endereco;
    SS_endereco << this;
    // JanelaMae->listBox_TarefasFrontPresentes->removeItemById(SS_endereco.str());
}

bool CtarefaFront::transfere()
{
//      cout << "TRANSFERE";
//   (*Cel)->popular_matriz_C(); //popula C e Z_0;

    int tamanho = altura_textura*largura_textura;
    CtarefaBack * tarefa_back;

    if (Programa->TarefasBackPresentes.empty())
    {
        new CtarefaBack(Programa);
    }

    tarefa_back = Programa->TarefasBackPresentes.back();
    TarefasBackPresentes.push_back(tarefa_back);
    (*Cel)->TarefasBackPresentes.push_back(tarefa_back);
    int * indice_destino = &tarefa_back->indice;

    tarefa_back->TarefasFrontPresentes.push_back(this);

    tarefa_back->indice_inicial.push_back(*indice_destino);
    for (int indice_origem = 0 ; indice_origem< tamanho; indice_origem++)
    {
        if ((*Cel)->pixel_aberto[i])
        {
            if ((*indice_destino) == tarefa_back->tamanho)  // caso a tarfe destinho encha
            {
                //Fecha a tarefa
                tarefa_back->indice_final.push_back(*indice_destino);
                tarefa_back->report();
                tarefa_back->tarefa_fechada = true;
                //Abre uma nova tarefa
                tarefa_back = new CtarefaBack(Programa);
                TarefasBackPresentes.push_back(tarefa_back);
                (*Cel)->TarefasBackPresentes.push_back(tarefa_back);
                tarefa_back->TarefasFrontPresentes.push_back(this);
                indice_destino = &tarefa_back->indice;
                tarefa_back->indice_inicial.push_back(*indice_destino);
            }
//            tarefa_back->Z_R[*indice_destino] = (*Cel)->Z_R[indice_origem];

                tarefa_back->Z_R[*indice_destino] = ((*Cel)->DadosCelula)[indice_origem].Z_R;
                tarefa_back->Z_I[*indice_destino] = ((*Cel)->DadosCelula)[indice_origem].Z_I;
                tarefa_back->C_R[*indice_destino] = ((*Cel)->DadosCelula)[indice_origem].C_R;
                tarefa_back->C_I[*indice_destino] = ((*Cel)->DadosCelula)[indice_origem].C_I;
                tarefa_back->indice_na_origem[*indice_destino] = indice_origem;
                (*indice_destino) ++;
            
        }
    }
    tarefa_back->indice_final.push_back(*indice_destino);
    status = Transferida;
    return true;
}




