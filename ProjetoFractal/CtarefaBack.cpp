
#include "CtarefaBack.h"


using namespace std;

CtarefaBack::CtarefaBack(Cprograma* Programa_in)
{
    Programa = Programa_in;
    if (Programa_in == nullptr) {
        cout << "PROGRAMA IN NULLPTR";
        getchar();
    }
    Programa->TarefasBackPresentes.push_back(this);

    //tamanho = tamanho_in;
    //i_max= i_max_in;

    stringstream SS_endereco;
    SS_endereco << this;
    string endereco = SS_endereco.str();
//       JanelaMae->listBox_TarefasFrontPresentes->addItem(SS_endereco.str(),endereco);
//atualiza_lista();
}


CtarefaBack::~CtarefaBack()
{
    //Programa->TarefasBackPresentes.remove(this);
    //   cout <<"\nDTAREFA " << this;
    stringstream SS_endereco;
    SS_endereco << this;
    //  JanelaMae->listBox_TarefasFrontPresentes->removeItemById(SS_endereco.str());

}

void CtarefaBack::report()
{
    unsigned int qtde_tarefas_presentes = TarefasFrontPresentes.size();
    if (qtde_tarefas_presentes != indice_inicial.size())
    {
        cout << "erro indices ";
        if (qtde_tarefas_presentes != indice_final.size())
        {
            cout << "no indice final";
            getchar();
        }
        else {cout << " no indice inicial"; getchar();}
    }
    for (unsigned int i = 0; i<qtde_tarefas_presentes; i++)
    {
        //    cout << "\nTB " << this << " contem " << TarefasFrontPresentes[i] << " de " << indice_inicial[i] << " até " << indice_final[i];


    }

}

void CtarefaBack::executa_cpu_simples()
{
    for (int i = 0; i<tamanho; i++)
    {
        //  cout << "\n computar C_R" << C_R[i];
        IT[i] = funcoes_mandelbrot::iteracoes_normalizada_int(C_R[i],C_I[i],i_max);
        //  IT[i]=i;
        //      cout << " resultou " << IT[i];

    }
}

void CtarefaBack::executa()
{
    executa_cpu_simples();
    transfere_para_Celulas();

}

void CtarefaBack::transfere_para_Celulas()
{

    int qtde_tarefas_presentes = TarefasFrontPresentes.size();
    if (qtde_tarefas_presentes == 0)
    {
        cout << "\nERRO ZERO TAREFAS FRONTEND PRESENTES NA TAREFA BACKEND";
        getchar();
    }

   // for (auto itrTarefaFront = TarefasFrontPresentes)
    for (int i_tarefa_front = 0; i_tarefa_front<qtde_tarefas_presentes; i_tarefa_front++)
    {
        TarefasFrontPresentes[i_tarefa_front]->TarefasBackPresentes.remove(this);
        TarefasFrontPresentes[i_tarefa_front]->TarefasBackPresentes.remove(this);
        TarefasFrontPresentes[i_tarefa_front]->TarefasBackPresentes.remove(this);
        Ccelula * Celula_destino = *TarefasFrontPresentes[i_tarefa_front]->Cel;
        if (Celula_destino != nullptr)
        {
            for (int i_pixel = indice_inicial[i_tarefa_front]; i_pixel < indice_final[i_tarefa_front]; i_pixel++)
            {//transfere os pixels
                //     Celula_destino->IT[indice_na_origem[i_pixel]] = IT[i_pixel];
                //  IT[i_pixel] = i_pixel;
              //  if (indice_na_origem[i_pixel]>=(altura_textura*largura_textura)){cout << "SEGFAULT";}
             //   if (i_pixel>=(altura_textura*largura_textura)){cout << "SEGFAULT";}


                Celula_destino->IT[indice_na_origem[i_pixel]] = IT[i_pixel];
                //      cout<< "\n it = " <<IT[i_pixel];
            }
            Celula_destino->resolucao_calculada = 1; // atribui
            Celula_destino->TarefasBackPresentes.remove(this);

            if  (Celula_destino->TarefasBackPresentes.size() == 0)
            {//verifica se ainda existem tarefas back na Celula
//                if(Celula_destino->esperandoTarefa ){
//                    cout << endl << "INCOSISNTENCIA" << endl;
//                    getchar();
//                }
               // Celula_destino->esperandoTarefa = false;
                Celula_destino->atualiza_textura();
                Celula_destino->EstaProntaParaExibir = true;
            }
        }
        else
        {
      //      cout << "\nTAREFA BACKEND APONTA PRA CELULA NULA ";
        }
        if  (TarefasFrontPresentes[i_tarefa_front]->TarefasBackPresentes.size() == 0)
            {//verifica se ainda existem tarefas back na tarefa front
                TarefasFrontPresentes[i_tarefa_front]->status=destruir;
            }
    }
}







