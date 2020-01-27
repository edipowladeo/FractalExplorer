#include "Cpaleta.h"

//class Cjanela;


   unsigned int Cpaleta::busca_paleta(int i,int posicao) //REMOVER ESSA FUNÇAO É MUITO FEIA
    {
        return *(Lista1Pal.ptrpal[i]+(posicao%Lista1Pal.tamanho[i]));
    }

    unsigned int Cpaleta::paleta_interpolada(double iterac,int indice_paleta,int interpolacao)
    {
        int cor1,cor2;
        //unsigned int red1,red2,green1,green2,blue1,blue2;
        unsigned int red,green,blue;
        double peso1, peso2;
        float ponto = iterac/interpolacao;

        cor1 = busca_paleta(indice_paleta-1,ponto);
        cor2 = busca_paleta(indice_paleta-1,ponto+1);

        peso2 = fmod(iterac,interpolacao);
        peso1 = 1 - peso2;

        //VER O QUE ACONTECE QUE OS PESOS FISub NEGATIVOS
        if (peso1<0) printf("\nHALT PESO1!!! %d",(int) peso1);
        if (peso2<0) return (0x00FF0000) + (OPACO) ;//printf("\nHALT PESO2!!! %d", peso2);

        //CORES ESTAO INVERTIDAS
        // Entrada RGB
        //SAIDA ABGR
        blue =        (peso1 * (unsigned char)(cor1) + peso2 * (unsigned char)(cor2))/interpolacao;
        green = 0.33+(peso1 * (unsigned char)(cor1>>8) + peso2 * (unsigned char)(cor2>>8))/interpolacao;
        red =  0.66+(peso1 * (unsigned char)(cor1>>16) + peso2 * (unsigned char)(cor2>>16))/interpolacao;

        if (red>255) printf("\nHALT RED!!! %d", red);
        if (green>255) printf("\nHALT RED!!! %d ", green);
        if (blue>255) printf("\nHALT RED!!! %d",blue);

//  if (indice_paleta ==4) printf("\nCOR 1 %x , cor 2 %x, posicao %d, cor resultado %x",cor1,cor2,peso1,((red) + (green<<8) + (blue <<16) + (0xff000000)));
  uint64_t alpha = 0xff000000;
        return (red) + (green<<8) + (blue <<16) + (alpha);
//red = (red2 * iterac%iterpolacao) + (red1 * (interpolacao - iterac%interpolacao)
    }

    unsigned int Cpaleta::paleta_quickman(int indice_paleta,int iterac)
    {
        int cor;
        unsigned int red,green,blue;
        cor = busca_paleta(indice_paleta%QTDE_PALETAS_QUICKMAN,iterac);
        blue =       cor % 256;
        green = (cor>>8) %256;
        red =  (cor>>16) %256;
        cor = (blue <<16) + (green<<8) + red + 0xff000000;
        return cor;
    }

    unsigned int Cpaleta::paleta(unsigned long int iterac_int,int indice_paleta) //retorna a cor; ja retorna o alpha correto
    {
        int  red, green, blue;
        double iterac;

        iterac = (double)iterac_int;
        if (!USAR_SHADER) iterac /= SAMPLING_ITERACOES;
        //  if (iterac == 0) return COR_INFINITO;
        if (indice_paleta==0)
        {
            red = 127.5+127*sin((float)iterac/30);
            green = 127.5+127*sin((float)iterac/30-2);
            blue = 127.5+127*sin((float)iterac/30+2);
uint64_t alpha = 0xff000000;
            return (red) + (green<<8) + (blue <<16) + (alpha);
        }
        else return paleta_quickman(indice_paleta-1,iterac);
    }

    int Cpaleta::tamanho_pal(int indice_paleta)
    {
        if (indice_paleta==0) return MAX_TAMANHO_PALETAS;
        else return Lista1Pal.tamanho[(indice_paleta-1)%QTDE_PALETAS_QUICKMAN];
    }

    unsigned int Cpaleta::paleta_precalculada_shader(unsigned long int iterac) //retorna a cor;
    {
        return paleta_pre_shader[(iterac)%(tamanho_pal(paleta_atual))];
    }

    unsigned int Cpaleta::paleta_precalculada(unsigned long int iterac) //retorna a cor;
    {
        return paleta_precalculada_shader(iterac);
    }

    void Cpaleta::precalcula_paleta() //PRECALCULA A PALETA TODA PARA OTIMIZAR EXIBIÇÃO
    {
        long int i;
        for(i=0; i<(tamanho_pal(paleta_atual)); i++)
        {
            paleta_pre_shader[i] = paleta(i,paleta_atual);
        }
    }
