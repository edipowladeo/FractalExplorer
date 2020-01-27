#ifndef CTAREFA_ALOCAR_CELULA_H
#define CTAREFA_ALOCAR_CELULA_H


class Ccelula;
class Cprograma;

#include "Csubcamada.h"
#include "Ccoordenadas.h"
#include "CPrograma.h"

class Csubcamada;

class CTarefaAlocarCelula
{
public:
    Csubcamada * SubCamada = nullptr;
    int i; //indices ptr_celulas
    int j;
    CcoordenadasPlano Coordenadas;
    Cprograma *Programa = nullptr;

    Ccelula ** PtrCelula = nullptr;

    bool TarefaFoiExecutada = false;

    CTarefaAlocarCelula(Csubcamada * SubCamada, int i,int j,CcoordenadasPlano Coordenadas,Cprograma *Programa);
    void executar();
};


#endif
