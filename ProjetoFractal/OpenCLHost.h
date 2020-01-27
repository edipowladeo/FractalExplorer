#ifndef OPENCLHOST_H
#define OPENCLHOST_H

#include<vector>


#include <stdio.h>
#include <stdlib.h>
#include <iostream>

#define CL_USE_DEPRECATED_OPENCL_1_2_APIS

#ifdef __APPLE__
#include <OpenCL/opencl.h>
#else
#include <CL/cl.h>
#endif

#define TAMANHO_VETOR 100

using namespace std;

struct Tdispostivo
{
    cl_device_id ID;
    cl_uint maxComputeUnits;
};

struct Tplataforma
{
    cl_platform_id ID;
    cl_uint deviceCount;
    cl_device_id *devices;
    vector<cl_device_id> Vdevices;
    vector<Tdispostivo> Dispositivos;
    cl_uint maxComputeUnits;
    char *value;
    size_t valueSize;
};

struct TListaPlataformas
{
    cl_uint QtdePlataformas;
    cl_platform_id *IdPlataformas;
    cl_int retorno;
   // Tplataforma **Plataformas;
    
    vector<Tplataforma> VPlataformas;

};

struct TFonte_Kernel
{
    char *source_str;
    size_t source_size;
};

class THost
{
public:
    signed int num_entradas;
    signed int num_saidas;
    signed int total_param;
    //  signed int dimensao_trabalho = 1;
    cl_device_id    Dispositivo_usar;
    cl_context contexto;
    cl_command_queue fila_comandos;
    cl_mem *objetos_memoria;
    cl_program programa;
    cl_kernel kernel;
    size_t global_work_size[1];
    cl_int retorno;
    struct TFonte_Kernel Fonte;
    unsigned int dim_trabalho;

    //lista plataformas

    int abrir_kernel_do_arquivo(TFonte_Kernel *Fonte);

    THost(cl_device_id *Dispositivo, signed int entradas, signed int saidas, const char funcao[32], signed int dimensao_trabalho);
    
    void Executar_kernel(float **vetores_entrada, float **vetores_saida);
    ~THost();
};

TListaPlataformas listarPlataformas();
void exibirDetalhesPlataformas(const TListaPlataformas &Plataformas);

int testeOpenCL();


#endif // OPENCLHOST_H
