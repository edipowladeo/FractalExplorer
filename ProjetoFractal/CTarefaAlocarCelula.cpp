#include "CTarefaAlocarCelula.h"

CTarefaAlocarCelula::CTarefaAlocarCelula(Csubcamada * SubCamada, int i,int j,CcoordenadasPlano Coordenadas,Cprograma *Programa)
{
    this->SubCamada = SubCamada;
    this->i=i;
    this->j=j;
    this->Programa = Programa;
    this->Coordenadas=Coordenadas;
   // SubCamada->TarefasAlocarCelulaPresentes.push_back(this);
    PtrCelula = SubCamada->PtrCelulas[i][j];
}

void CTarefaAlocarCelula::executar()
{
  //  cout << endl << "EX " << i << " " << j << "sub " << SubCamada;
//    if (*SubCamada->PtrCelulas[i][j] == nullptr)
//    {
//        *SubCamada->PtrCelulas[i][j] = new Ccelula(SubCamada,Coordenadas);
//
//            (*SubCamada->PtrCelulas[i][j])->tarefa = new CtarefaFront(SubCamada,i,j,1,Programa);
//
//    }
//    else
//    {
//        cout << "ERRO CTarefaAlocarCelula::Executar() , celula já está alocada";
//        getchar();
//    }
    Ccelula * Celula = *PtrCelula;

     if (Celula == nullptr){
                    //     cout << "CELULA == nullptr em CTarefaAlocarCelula::executar()";
                         //  throw new IllegalStateException("this should never happen: CELULA == nullptr em CTarefaAlocarCelula::executar()");
                      //   getchar();

     }
     else {
         if (Coordenadas == Celula->coordenadas_canto) {
             //cout << endl << Celula->JanelaMae;
             //Celula->ReadSomething();
             Celula->AlocarVetoresDeDados();
             
         }
         else { cout << endl << "mismatch coordinates"; getchar(); }
     }
//        catch (const std::segfault){
//        cout << "\nSEGFAULT TAREFA\N";
//        getchar();
//        }


    TarefaFoiExecutada=true;
}
