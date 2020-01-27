#ifndef CCRONOMETRO_H
#define CCRONOMETRO_H

#include <chrono>

class CCronometro
{
    long long int inicio=0;
    long long int fim=0;
    long long int sum=0;
    public:
    int voltas=0;
public:
   void inicia()
    {
        inicio = std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::high_resolution_clock::
                 now().time_since_epoch()).count();
    }
   void para()
    {
        fim = std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::high_resolution_clock::
                now().time_since_epoch()).count();
    }
   long long int tempoagora()
   {
       long long int tempoagora = std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::high_resolution_clock::
           now().time_since_epoch()).count();
       return tempoagora - inicio;
   }
   void volta() {voltas++; sum+=fim-inicio;}
   void paraeVolta() {para(); volta();}
   void paraVoltaInicia() {para(); volta(); inicia();}
    long long int media()
    {
        if (voltas ==0 ) return 0;
        return sum/voltas;
    }
    void reinicia()
    {
        sum=0;
        inicio=0;
        fim=0;
        voltas=0;
    }

};

# endif
