#include "csubcamada.h"

    Cvetor2d Csubcamada::coordenadas_plano_para_indice(CCoordenadas2DPlano Coordenadas_plano)
    {
        Cvetor2d indice;
        indice.x =  Coordenadas_plano.x -Coordenadas.x;
        indice.x /= Coordenadas.delta;
        indice.x /= (largura_textura-SOBREPOSICAO);
        indice.x += (float)COLUNAS_DE_SPRITES/2;
        indice.x += offset_i;

        indice.y =  Coordenadas_plano.y -Coordenadas.y;
        indice.y /= Coordenadas.delta;
        indice.y /= (altura_textura-SOBREPOSICAO);
        indice.y += (float)LINHAS_DE_SPRITES/2;
        indice.y += offset_j;

        return indice;
    }

    Csubcamada::Csubcamada(Cjanela * Janela,long int log2delta,CcoordenadasPlano Coordenadas)
    {
        JanelaMae = Janela;
        int i,j;
        for (i = 0; i<COLUNAS_DE_SPRITES; i++)
        {
            for (j = 0; j<LINHAS_DE_SPRITES; j++)
            {
                Celulas[i][j] = nullptr;
                PtrCelulas[i][j] = &Celulas[i][j];;
            }
        }
        this->log2delta = log2delta;

        Coordenadas.delta = pow(2,-log2delta);//Janela->DeltaDesejado; //REMOVER
     //   cout << "criando camada com delta " <<  Coordenadas.delta << " e logd2 " << log2delta<<endl;
        Coordenadas.delta = pow(2,-log2delta);

        this->Coordenadas = Coordenadas;
        //Coordenadas = Janela->pivoDesejado, chama copy constructor de  acima deletes .delta


      //  if (Janela->Ultima_Subcamada == nullptr) Janela->Ultima_Subcamada=this;


        stringstream SS_endereco;
        SS_endereco << this << "-" << this-> log2delta;
        string endereco = SS_endereco.str();
        SS_endereco << " Delta " <<  Coordenadas.delta;
    }



    Csubcamada::~Csubcamada()
    {
     //   std::cout << "\nDELETOU " << this;

        stringstream SS_endereco;
        SS_endereco << this;
    }


void Csubcamada::deletarCelula(Ccelula* Celula){


}

void Csubcamada::atualiza_textura()
{
    int i,j;
    Ccelula *Cel;
    for (i = 0; i<COLUNAS_DE_SPRITES; i++)
    {
        for (j = 0; j<LINHAS_DE_SPRITES; j++)
        {
            Cel = *PtrCelulas[i][j];
            if(Cel!=nullptr)
            {
                Cel->atualiza_textura();
            }
        }
    }
}

void Csubcamada::posiciona_celulas_tela() //POSICAO NA TELA DE ACORDO COM AS COORDENADAS DE centro.x[C] E MAGNIFICACAO DA Janela->Camera ATUAL
{
    double escala = Coordenadas.delta/JanelaMae->DeltaAtual;
    for (int i = 0; i<COLUNAS_DE_SPRITES; i++)
    {
        for (int j = 0; j<LINHAS_DE_SPRITES; j++)
        {
            if (*PtrCelulas[i][j]!= nullptr)
            {
                Ccelula *Cel = *PtrCelulas[i][j];
                CCoordenadas2DTela Posicao_tela = JanelaMae->CoordenadasPlanoAtualParaTela(Cel->coordenadas_canto);
                Cel->sprite.setScale(escala,escala);
                Cel->sprite.setPosition(Posicao_tela.x,Posicao_tela.y);
            }
        }
    }
}


CIntervalo2DI Csubcamada::CalculaIntervaloIJ(const CCoordenadas2DTela & CoordCentro, const sf::Vector2f & janela)
{
    float delta = Coordenadas.delta / JanelaMae->DeltaDesejado;

    CIntervalo2DI Intervalo;

    Intervalo.max.x = floor((janela.x / 2 - CoordCentro.x) / delta / (largura_textura - SOBREPOSICAO)) + COLUNAS_DE_SPRITES / 2;
    Intervalo.max.y = floor((janela.y / 2 - CoordCentro.y) / delta / (altura_textura - SOBREPOSICAO)) + LINHAS_DE_SPRITES / 2; 
    
    Intervalo.min.x = floor((-janela.x / 2 - CoordCentro.x) / delta / (largura_textura - SOBREPOSICAO)) + COLUNAS_DE_SPRITES / 2;
    Intervalo.min.y = floor((-janela.y / 2 - CoordCentro.y) / delta / (altura_textura - SOBREPOSICAO)) + LINHAS_DE_SPRITES / 2;

    Intervalo.max.x = min(Intervalo.max.x, COLUNAS_DE_SPRITES);
    Intervalo.max.y = min(Intervalo.max.y, LINHAS_DE_SPRITES);
    
    Intervalo.min.x = max(Intervalo.min.x, 0);
    Intervalo.min.y = max(Intervalo.min.y, 0);

    return Intervalo;
}


void Csubcamada::CalculaIntervalos() {

    CCoordenadas2DTela CoordCentroSubcamadaTela = JanelaMae->CoordenadasPlanoAtualParaTela(Coordenadas);

    //escala das Células
    float delta = Coordenadas.delta / JanelaMae->DeltaDesejado;

    //coordenadas da camada na tela, em relação ao centro da tela
    CCoordenadas2DTela CoordSubcamadaRelativaAoCentroTela;
    CoordSubcamadaRelativaAoCentroTela.x = (CoordCentroSubcamadaTela.x - JanelaMae->Propriedades.DimensoesJanela.x / 2);// / (largura_textura - SOBREPOSICAO));
    CoordSubcamadaRelativaAoCentroTela.y = (CoordCentroSubcamadaTela.y - JanelaMae->Propriedades.DimensoesJanela.y / 2);// / (altura_textura - SOBREPOSICAO));

    IntervaloAlocar = CalculaIntervaloIJ(CoordSubcamadaRelativaAoCentroTela, JanelaMae->Propriedades.JanelaAlocar);
    IntervaloDesenhar = CalculaIntervaloIJ(CoordSubcamadaRelativaAoCentroTela, JanelaMae->Propriedades.JanelaDesenho);
    IntervaloDesalocar = CalculaIntervaloIJ(CoordSubcamadaRelativaAoCentroTela, JanelaMae->Propriedades.JanelaDesalocar);
}






void    Csubcamada::MarcarCelulasInuteis(){
    Ccelula *Cel;
bool  Subcamada_vazia=true;
        for (int i = 0; i<COLUNAS_DE_SPRITES; i++)
        {
            for (int j = 0; j<LINHAS_DE_SPRITES; j++)
            {
                Cel = *PtrCelulas[i][j];
                if (Cel!= nullptr)
                {
                    Subcamada_vazia=false;
                    if (!JanelaMae->celula_esta_dentro(Cel,JanelaMae->Propriedades.JanelaDesalocar))
                    {
                        (*PtrCelulas[i][j])->marcarParaDestruicao();
                         *PtrCelulas[i][j] = nullptr;
                    }

                    /*      else        //CASO ESTEJA DENTRO FAZER A MAGICA
                          {

                              //    printf("\nmagica começou");

                              Cretangulo Indice_ultima_subcamada;
                              CCoordenadas2DPlano Coordenadas_canto_inferior_direito;
                              Coordenadas_canto_inferior_direito = Cel->coordenadas_canto;
                              Coordenadas_canto_inferior_direito.x += (largura_textura-SOBREPOSICAO)*Cel->coordenadas_canto.delta;
                              Coordenadas_canto_inferior_direito.y += (altura_textura-SOBREPOSICAO)*Cel->coordenadas_canto.delta;
                              Indice_ultima_subcamada.minimo = Janela->Ultima_Subcamada->coordenadas_plano_para_indice(Cel->coordenadas_canto);
                              Indice_ultima_subcamada.maximo = Janela->Ultima_Subcamada->coordenadas_plano_para_indice(Coordenadas_canto_inferior_direito);

                              int pesquisa_i;
                              int pesquisa_j;


                              sf::Vector2i pesquisa_min, pesquisa_max;
                              int mx = Indice_ultima_subcamada.minimo.x;
                              int my = Indice_ultima_subcamada.minimo.y;
                              int Mx = Indice_ultima_subcamada.maximo.x;
                              int My = Indice_ultima_subcamada.maximo.y;
                              pesquisa_min.x = max(mx, 0);
                              pesquisa_min.y = max(my, 0);
                              pesquisa_max.x = min(Mx, COLUNAS_DE_SPRITES);
                              pesquisa_max.y = min(My, LINHAS_DE_SPRITES);

                              int pode_apagar= true;
                              pode_apagar = false; // desativa toda afuncao;
                              int pesquisou_ao_menos_uma_celula = false;
                              for(pesquisa_i = pesquisa_min.x; pesquisa_i< pesquisa_max.x; pesquisa_i++)
                              {
                                  for(pesquisa_j = pesquisa_min.y; pesquisa_j< pesquisa_max.y; pesquisa_j++)
                                  {
                                      Ccelula* Celula_pesquisar;
                                      Celula_pesquisar = *(Janela->Ultima_Subcamada->PtrCelulas[pesquisa_i][pesquisa_j]);
                                      if (Celula_pesquisar == nullptr)
                                      {
                                          //  printf("\n PESQUISANDO CELULA nullptr pressione qualquer tecla para continuar (travar)");
                                          //   getchar();
                                          pode_apagar = false;

                                      }
                                      else
                                      {
                                          pesquisou_ao_menos_uma_celula = true;
                                          if (Celula_pesquisar->resolucao_calculada*Celula_pesquisar->coordenadas_canto.delta >
                                                  Cel->resolucao_calculada*Cel->coordenadas_canto.delta)
                                          {
                                              pode_apagar = false;
                                          }
                                      }
                                  }
                              }
                              //   printf ("\n checkponi");
                              if ((pode_apagar))
                              {
                                  //SE A CELULA false ACHOU CELULAS COM MAIOR RESOLUCAO

                                                     (*Subcamada_atual->PtrCelulas[i][j])->marcarParaDestruicao();
                                                     Subcamada_atual->PtrCelulas[i][j] = nullptr;
                              }
                              //    printf("\nmagica terminou");

                          }

                    */

                }
            }
        }
        if(Subcamada_vazia)
        {
            //    delete ((*CAMADA_ATUAL).second);
            //    Janela->Camadas.erase(CAMADA_ATUAL);
            // exemplo check past end it >>> if (ItrTarefa == Camada->TarefasFrontPresentes.end()) break;

            // check past end it
            cout << endl << "SUBCAMADA VAZIA" << endl;
        }
    }

