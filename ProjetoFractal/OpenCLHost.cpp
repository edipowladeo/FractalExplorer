#include "OpenCLHost.h"

/* PENDENCIAS E NOTAS
    #define MAX_SOURCE_SIZE (0x100000) // QUAL A UTILIDADE DISSO? N TEM COMO VER O TAMANHO DO ARQUIVO?
    PODE DAR PROBLEMA COM KERNELS MT GRANDES

qUAL A VANTAGEM E COMO IMPLEMENTAR A CRIAÇÃO DE PROGRAMA SEM DISPOSITIVO ATRELADO
retorno = clBuildProgram(program, 0, NULL,NULL,NULL,NULL);

melhorar a classe nas passagem de variaveis source string e source size

// NO MOMENTO A CLASSE SÓ ACEITA TRABALHOS UNIDIMENSIONAIS size_t global_work_size[1];

tratar erros

free e delete dentro da lista plataformas;

*/



    int THost::abrir_kernel_do_arquivo(TFonte_Kernel *Fonte)
    {
        FILE *ptr_arquivo;
        char fileName[] = "./KernelsOpenCL/opencl_kernels.cl";
        char *source_str;
        size_t source_size;

        /* Load the source code containing the kernel*/
         fopen_s(&ptr_arquivo, fileName, "r");
//        ptr_arquivo = nullptr;
        if(!ptr_arquivo)
        {
            fprintf(stderr, "Failed to load kernel.\n");
            getchar();
            return 0;
        }

#define MAX_SOURCE_SIZE (0x100000) // QUAL A UTILIDADE DISSO?
        Fonte->source_str = (char *)malloc(MAX_SOURCE_SIZE); // n tem como fazer um sizeof algo assim????
        Fonte->source_size = fread(Fonte->source_str, 1, MAX_SOURCE_SIZE, ptr_arquivo);
        fclose(ptr_arquivo);
        return 1;
    }

    THost::THost(cl_device_id *Dispositivo, signed int entradas, signed int saidas,const char funcao[32], signed int dimensao_trabalho)
    {
        num_entradas = entradas;
        num_saidas = saidas;
        total_param = entradas + saidas;
        dim_trabalho = dimensao_trabalho;
        Dispositivo_usar = *Dispositivo;

        abrir_kernel_do_arquivo(&Fonte);
        //contexto 1 dispositivo e ponteiro deste dispositivo
        contexto = clCreateContext(NULL, 1, &Dispositivo_usar, NULL, NULL, &retorno);

        fila_comandos = clCreateCommandQueue(contexto, Dispositivo_usar, 0, &retorno);
        // create the program
        programa = clCreateProgramWithSource(contexto, 1, (const char **)&Fonte.source_str, (const size_t *)&Fonte.source_size, &retorno);
        // build the program 
        retorno = clBuildProgram(programa, 1, &Dispositivo_usar, NULL, NULL, NULL);
       
        // create the kernel
        kernel = clCreateKernel(programa, funcao, &retorno);
        // set work-item dimensions
        global_work_size[0] = dimensao_trabalho;

        objetos_memoria = (cl_mem *)malloc(sizeof(cl_mem) * (total_param));
    }


    void THost::Executar_kernel(float **vetores_entrada, float **vetores_saida)
    {
        int in, out, i;
        
        cl_int retorno;
        // allocate the buffer memory objects
        for(in = 0; in < num_entradas; in++)
        {
            objetos_memoria[in] = clCreateBuffer(contexto, CL_MEM_READ_ONLY | CL_MEM_USE_HOST_PTR, sizeof(cl_float) *
                dim_trabalho, vetores_entrada[in], &retorno);

           // retorno = clEnqueueWriteBuffer(fila_comandos, objetos_memoria[in], CL_TRUE, 0, sizeof(cl_float) * dim_trabalho, A, 0, NULL, NULL);

            printf("ret at %d is %d\n", __LINE__, retorno);
        }

        for(out = 0; out < num_saidas; out++)
        {
            objetos_memoria[in + out] = clCreateBuffer(contexto, CL_MEM_WRITE_ONLY | CL_MEM_USE_HOST_PTR, sizeof(cl_float) * dim_trabalho, vetores_saida[out],&retorno);
            printf("ret at %d is %d\n", __LINE__, retorno);
        }



        // set the args values
        for(i = 0; i < total_param; i++)
        {
            retorno = clSetKernelArg(kernel, i, sizeof(cl_mem), (void *)&objetos_memoria[i]);
        }

        // execute kernel
        cl_event *Este_evento; //salva evento no ponteiro ESTE evento, para sincronização
        retorno = clEnqueueNDRangeKernel(fila_comandos, kernel, 1, NULL, global_work_size, NULL, 0, NULL, NULL);

        // read output array
        for(out = 0; out < num_saidas; out++)
        {
            retorno = clEnqueueReadBuffer(fila_comandos, objetos_memoria[in + out], CL_TRUE, 0, (dim_trabalho) * sizeof(cl_float), vetores_saida[out], 0, NULL, NULL);
        }
    }

    THost::~THost()
    {
        retorno = clFlush(fila_comandos);
        retorno = clFinish(fila_comandos);
        retorno = clReleaseKernel(kernel);
        retorno = clReleaseProgram(programa);
        retorno = clReleaseMemObject(objetos_memoria[0]);
        retorno = clReleaseMemObject(objetos_memoria[1]);
        retorno = clReleaseMemObject(objetos_memoria[2]);
        retorno = clReleaseCommandQueue(fila_comandos);
        retorno = clReleaseContext(contexto);
        free(objetos_memoria);
        free(Fonte.source_str);
    }


    



    void exibirDetalhesPlataformas(const TListaPlataformas & Plataformas)
    {
        int i = 1;
        for(auto ItrPlataforma = Plataformas.VPlataformas.begin(); ItrPlataforma != Plataformas.VPlataformas.end(); ItrPlataforma++)
        {
            int j = 1;
            cout << endl<<endl << "PLATAFORMA " << i++ << ":"<<endl;
            for(auto ItrDispositivo = (*ItrPlataforma).Dispositivos.begin(); ItrDispositivo != (*ItrPlataforma).Dispositivos.end(); ItrDispositivo++)
            {
                size_t TamValor = 0;
                char *valor;

                // print device name
                clGetDeviceInfo((*ItrDispositivo).ID, CL_DEVICE_NAME, 0, NULL, &TamValor);
                valor = (char *)malloc(TamValor);
                clGetDeviceInfo((*ItrDispositivo).ID, CL_DEVICE_NAME, TamValor, valor, NULL);
                cout << j << ". Device:\t" << valor << endl;
                free(valor);

                // print hardware device version
                clGetDeviceInfo((*ItrDispositivo).ID, CL_DEVICE_VERSION, 0, NULL, &TamValor);
                valor = (char *)malloc(TamValor);
                clGetDeviceInfo((*ItrDispositivo).ID, CL_DEVICE_VERSION, TamValor, valor, NULL);
                cout << j  << ".1 Hardware version:\t" << valor << endl;
                free(valor);

                // print software driver version
                clGetDeviceInfo((*ItrDispositivo).ID, CL_DRIVER_VERSION, 0, NULL, &TamValor);
                valor = (char *)malloc(TamValor);
                clGetDeviceInfo((*ItrDispositivo).ID, CL_DRIVER_VERSION, TamValor, valor, NULL);
                cout << j  << ".2 Software version:\t" << valor << endl;
                free(valor);

                // print c version supported by compiler for device
                clGetDeviceInfo((*ItrDispositivo).ID, CL_DEVICE_OPENCL_C_VERSION, 0, NULL, &TamValor);
                valor = (char *)malloc(TamValor);
                clGetDeviceInfo((*ItrDispositivo).ID, CL_DEVICE_OPENCL_C_VERSION, TamValor, valor, NULL);
                cout << j << ".3 OpenCL C version:\t" << valor << endl;
                free(valor);

                cout << j  << ".4 Parallel compute units:\t" << (*ItrPlataforma).maxComputeUnits << endl;
                
                j++;
            }
        }
    }

    TListaPlataformas listarPlataformas()
    {
        TListaPlataformas ListaP;

        // get all platforms
        clGetPlatformIDs(0, NULL, &ListaP.QtdePlataformas);

        cl_platform_id *IdPlataformas = (cl_platform_id *)malloc(sizeof(cl_platform_id) * ListaP.QtdePlataformas);
        clGetPlatformIDs(ListaP.QtdePlataformas, IdPlataformas, NULL);

        ListaP.VPlataformas.resize(ListaP.QtdePlataformas);

        for(int i = 0; i < ListaP.QtdePlataformas; i++)
        {
            ListaP.VPlataformas[i].ID = IdPlataformas[i];

            clGetDeviceIDs(ListaP.VPlataformas[i].ID, CL_DEVICE_TYPE_ALL, 0, NULL, &ListaP.VPlataformas[i].deviceCount);
            cl_device_id *IdDevices = (cl_device_id *)malloc(sizeof(cl_device_id) * ListaP.VPlataformas[i].deviceCount);
            clGetDeviceIDs(ListaP.VPlataformas[i].ID, CL_DEVICE_TYPE_ALL, ListaP.VPlataformas[i].deviceCount, IdDevices, NULL);

            ListaP.VPlataformas[i].Dispositivos.resize(ListaP.VPlataformas[i].deviceCount);
            for(int j = 0; j < ListaP.VPlataformas[i].deviceCount; j++)
            {
                ListaP.VPlataformas[i].Dispositivos[j].ID = IdDevices[j];

                // Get parallel compute units
                clGetDeviceInfo(ListaP.VPlataformas[i].Dispositivos[j].ID , CL_DEVICE_MAX_COMPUTE_UNITS, sizeof(ListaP.VPlataformas[i].maxComputeUnits), &ListaP.VPlataformas[i].maxComputeUnits, NULL);
            }
            free(IdDevices);
        }
        free(IdPlataformas);
        return ListaP;
    }

int testeOpenCL()
{
    
    //INICIO
    TListaPlataformas Lista = listarPlataformas();
    cl_device_id Dispositivo1  = Lista.VPlataformas[1].Dispositivos[0].ID;

 ///  Tdispostivo = ListaP.VPlataformas[0].Dispositivos[0];
    const char * nome = "vadd";
    THost *Host = new THost(&Dispositivo1, 3, 1, nome, TAMANHO_VETOR);

    //PREPARAR VETORES NO HOST
    int tamanho_vetor = TAMANHO_VETOR;

    float vetor_A[TAMANHO_VETOR], vetor_B[TAMANHO_VETOR], vetor_C[TAMANHO_VETOR], vetor_D[TAMANHO_VETOR];
    float *Vetores_entrada[3];
    float *Vetores_saida[1];

    Vetores_entrada[0] = vetor_A;
    Vetores_entrada[1] = vetor_B;
    Vetores_entrada[2] = vetor_C;
    Vetores_saida[0] = vetor_D;

    for(int j = 0; j < 2; j++)

    {
        for(int i = 0; i < TAMANHO_VETOR; i++)
        {
            vetor_A[i] = j + rand() / (float)RAND_MAX;
            vetor_B[i] = j + rand() / (float)RAND_MAX;
            vetor_C[i] = 8000;
        }
    

        Host->Executar_kernel(&Vetores_entrada[0], &Vetores_saida[0]);

        //EXIBIR RESULTADOS
        for(int i = 0; i < TAMANHO_VETOR; i++)
        {
            //   printf("\n%f + %f = %f",vetor_A[i],vetor_B[i],vetor_C[i]);
            printf("\n%f + %f = %f", vetor_A[i], vetor_B[i], vetor_D[i]);
        }
    }

    delete(Host);



    printf("\nNAO DEU PAU");
    return 0;


}
