#include "Ccelula.h"


Ccelula::Ccelula(Csubcamada * Sub,CcoordenadasPlano Coordenadas)
{
    

    //    unique_lock<mutex> TravaJanela(JanelaMae->Trava);
    SubcamadaMae = Sub;
    JanelaMae = SubcamadaMae->JanelaMae;


    UsandoShader = JanelaMae->Propriedades.UsarShader;
//      unique_lock<mutex> TravaJanela(JanelaMae->Trava);
    int linhas_adicionais_textura = 0;

    if (ShaderDisponivel)  //preparar textura para paleta do shader
    {
        linhas_adicionais_textura =  LINHAS_ADICIONAIS_TEXTURA;
    }
    JanelaMae->Programa->CronometroCriarTextura.inicia();
    textura.create(largura_textura,altura_textura+linhas_adicionais_textura);
    textura.setSmooth(JanelaMae->smooth);
    sprite.setTexture(textura);
    JanelaMae->Programa->CronometroCriarTextura.paraeVolta();
    bitmap_char = ((sf::Uint8*) &(bitmap));
    coordenadas_canto = Coordenadas;

    JanelaMae->ListaCelulasVivas.push_front(this);

    //  printf("\n celula alocada x %f y%f", Coordenadas.Pos.x,Coordenadas.Pos.y);
}

Ccelula::~Ccelula()
{


 //  if (DadosCelula != nullptr){
       // DadosCelula->clear();
  //      cout <<"\n VAI DELETAR \n";// getchar();
   //if(DadosCelula==nullptr) delete DadosCelula;
   //delete DadosCelula;

   //}
}

void Ccelula::marcarParaDestruicao()
{
    marcadaParaDestruicao = true;
}

void Ccelula::textura_matriz_int_res_arbitr() //MEDIR VELOCIDADE E OTIMIZAR;
{
    int x,y,i,j,pidex;
    unsigned long int cor;
    int totalint = 0;
    float total = 0;
    for (y=0; y<altura_textura; y+=resolucao_calculada)
    {
        for (x=0; x<largura_textura; x+=resolucao_calculada)
        {
            pidex = x + y* largura_textura;
            cor = JanelaMae->Paleta.paleta_precalculada(matriz_iteracoes[x][y]);
            total += IT[pidex];
            totalint += IT[pidex];
            for (i=0; i<resolucao_calculada; i++)
            {
                for (j=0; j<resolucao_calculada; j++)
                {
                    pidex = x+i + (y+j)*largura_textura;
                    bitmap[pidex] = cor;
                }
            }
        }
    }
//        Benchmark1.setscore(1,totalint,total);
    textura.update(bitmap_char);
}

void Ccelula::textura_matriz_float_res_arbitr() //MEDIR VELOCIDADE E OTIMIZAR;
{
    int x,y,i,j,pidex;
    unsigned long int cor;
    int totalint = 0;
    float total = 0;
    for (y=0; y<altura_textura; y+=resolucao_calculada)
    {
        for (x=0; x<largura_textura; x+=resolucao_calculada)
        {
            pidex = x + y * largura_textura;
            total += IT[pidex];
            totalint += IT[pidex];
            //cor = JanelaMae->Paleta.paleta_precalculada(IT[pidex]);
                cor = JanelaMae->Paleta.paleta_precalculada(IT[pidex]/SAMPLING_ITERACOES);

            // cout << "\n cor  " << cor ;
            for (i=0; i<resolucao_calculada; i++)
            {
                for (j=0; j<resolucao_calculada; j++)
                {
                    pidex = x+i + (y+j)*largura_textura;
                    bitmap[pidex] = cor;
                }
            }
        }
    }
//        Benchmark1.setscore(1,totalint,total);
    textura.update(bitmap_char);
}

void Ccelula::textura_shader_matriz_float_res_arbitr() //MEDIR VELOCIDADE E OTIMIZAR;
{
    int x,y,i,j,pidex;
    unsigned long int valor;
    int comprimento_textura = altura_textura*largura_textura;
//    int totalint = 0;
//    float total = 0;
    for (y=0; y<altura_textura; y+=resolucao_calculada)
    {
        for (x=0; x<largura_textura; x+=resolucao_calculada)
        {
            //  total += IT[pidex];
            // totalint += IT[pidex];
            pidex = x + y*largura_textura;
            valor = (IT[pidex]);//*SAMPLING_ITERACOES);
            //  cout << "\n valor " << valor;
//                valor = (IT[pidex]*SAMPLING_ITERACOES);
            for (i=0; i<resolucao_calculada; i++)
            {
                for (j=0; j<resolucao_calculada; j++)
                {
                    pidex = x+i + (y+j)*largura_textura;
                    bitmap[pidex] = (int) valor;
                }
            }
        }
    }



    for (pidex=0; pidex<(JanelaMae->Paleta.tamanho_pal(JanelaMae->Paleta.paleta_atual)); pidex++) //passa paleta
    {
        bitmap[pidex+comprimento_textura] = JanelaMae->Paleta.paleta_precalculada(pidex);
    }
    //   Benchmark1.setscore(1,totalint,total);
    textura.update(bitmap_char);
}

void  Ccelula::textura_shader_matriz_float_res1() //MEDIR VELOCIDADE E OTIMIZAR;
{
    int pidex;
    int comprimento_textura = altura_textura*largura_textura;
    for (pidex=0; pidex<comprimento_textura; pidex++) // passa iteracoes
    {
        bitmap[pidex] = (int)(IT[pidex]);//*SAMPLING_ITERACOES);
    }

    for (pidex=0; pidex<(JanelaMae->Paleta.tamanho_pal(JanelaMae->Paleta.paleta_atual)); pidex++) //passa paleta
    {
        bitmap[pidex+comprimento_textura] = JanelaMae->Paleta.paleta_precalculada(pidex);
    }
    textura.update((bitmap_char));
}

void  Ccelula::textura_matriz_float_res1() //MEDIR VELOCIDADE E OTIMIZAR;
{
    int pidex;
    float total = 0;
    int totalint = 0;
    int comprimento_textura = altura_textura * largura_textura;
//    unsigned long int cor;
    for (pidex=0; pidex<comprimento_textura; pidex++)
    {
        total += IT[pidex];
        totalint += IT[pidex];
        bitmap[pidex] = JanelaMae->Paleta.paleta_precalculada((int)(IT[pidex]));//*SAMPLING_ITERACOES));
        bitmap[pidex] = 0xffffffff;
    }

    // cout << "\n"<< total;
//        Benchmark1.setscore(1,totalint,total);
    textura.update((bitmap_char));
}

void Ccelula::ReadSomething() {
    Cjanela* J = JanelaMae;
    if (JanelaMae == nullptr) {
        cout << "NULL JANELA"; getchar();
    }
        if ((int)JanelaMae == 0xdddddddddddddddd) {
            cout << JanelaMae;
            cout << "NULL DEAD MEMORY"; getchar();
    }
}
void Ccelula::AlocarVetoresDeDados()
{
try {
   // DadosCelula= new vector<CDadosCelula>(largura_textura*altura_textura);
    DadosCelula.resize(largura_textura * altura_textura);
} 
catch (const std::bad_alloc){
        cout << "\nBAD ALLOC Ccelula::AlocarVetoresDeDados()\n new vector<CDadosCelula>( ";
        getchar();
        }

    TCoordenadas x0 = (coordenadas_canto.x);
    TCoordenadas y0 = (coordenadas_canto.y);
    for (int x=0; x<largura_textura; x++)
    {
        for (int y=0; y<altura_textura; y++)
        {
                int pidex = x+y*largura_textura;
      
               // assert(pidex < largura_textura * altura_textura);
                (DadosCelula)[pidex].C_I = y0 + y*coordenadas_canto.delta;
                 (DadosCelula)[pidex].C_R = x0 + x*coordenadas_canto.delta;
                 (DadosCelula)[pidex].Z_I=0;
                 (DadosCelula)[pidex].Z_R=0;
                 (DadosCelula)[pidex].IT= 0;
                 pixel_aberto[pidex] = true;
        }
    }
}



void Ccelula::atualiza_textura()
{
    if (UsandoShader)
    {
        int opencl_ativo = 0;
        if (opencl_ativo)
            textura_shader_matriz_float_res1();
        else textura_shader_matriz_float_res_arbitr(); //MEDIR VELOCIDADE E OTIMIZAR;
    }
    else
    {
        int opencl_ativo = 0;
        if (opencl_ativo) textura_matriz_float_res1();
        else textura_matriz_float_res_arbitr();
    }
}
